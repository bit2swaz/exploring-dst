// milestone 7: same kind of fault test against a small real tokio service
//
// in milestones 1 to 6 we wrote the simulator by hand. here `turmoil` (from the tokio team) is
// the simulator. it gives each host a fake network and a fake clock and it runs all hosts on
// one thread w one seed. the service code is normal async tokio code. the only change for
// the simulation is the import, which is `turmoil::net::UdpSocket` in place of `tokio::net::UdpSocket`
//
// we use UDP because a lost UDP datagram is just lost like in our own fake network
// (turmoil's TCP never sends a lost segment again so TCP faults there do not act like real TCP)

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::rc::Rc;
use std::time::Duration;
use turmoil::net::UdpSocket;

const PORT: u16 = 9000;
const FOLLOWERS: [&str; 2] = ["f1", "f2"];
const KEYS: [&str; 3] = ["a", "b", "c"];
const OPS: u64 = 10;

/// a store that the test can read after the run
/// Rc + RefCell (no locks) is alright because turmoil runs every host on one thread
type Store = Rc<RefCell<BTreeMap<String, u64>>>;

// ---------- the service ----------

/// one node. it receives datagrams forever:
/// - "PUT key val" (from the client): save it then send "REP key val" to each peer
/// - "REP key val" (from the leader): save it
/// same naive design as milestone 3: no version numbers, no acks, no retries
async fn node(store: Store, peers: &'static [&'static str]) -> turmoil::Result {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, PORT)).await?;
    let mut buf = [0u8; 64];
    loop {
        let (n, _from) = socket.recv_from(&mut buf).await?;
        let text = std::str::from_utf8(&buf[..n])?;
        let parts: Vec<&str> = text.split(' ').collect();
        let (kind, key, val) = (parts[0], parts[1], parts[2].parse::<u64>()?);

        store.borrow_mut().insert(key.to_string(), val);
        if kind == "PUT" {
            for peer in peers {
                socket.send_to(format!("REP {key} {val}").as_bytes(), (*peer, PORT)).await?;
            }
        }
    }
}

// ---------- the simulation ----------

/// the network for one run
struct Net {
    fail_rate: f64,         // chance (per message) that a link breaks. while broken, it drops everything
    max_latency_ms: u64,    // each message takes 1..=max ms. max > 1 means messages can reorder
}

const PERFECT: Net = Net { fail_rate: 0.0, max_latency_ms: 1 };
const BAD: Net = Net { fail_rate: 0.05, max_latency_ms: 100 };

/// run one simulation. returns the 3 stores at the end: [leader, f1, f2]
fn run(seed: u64, net: &Net) -> Vec<BTreeMap<String, u64>> {
    let mut sim = turmoil::Builder::new()
        .rng_seed(seed)     // all of turmoil's luck comes from this seed
        .fail_rate(net.fail_rate)
        .repair_rate(0.5)   // chance (per message) that a broken link heals
        .min_message_latency(Duration::from_millis(1))
        .max_message_latency(Duration::from_millis(net.max_latency_ms))
        .build();

    let stores: Vec<Store> = (0..3).map(|_| Store::default()).collect();

    // a host closure can run more than one time (turmoil can restart a crashed host),
    // so it gives a new future on each call
    let leader = stores[0].clone();
    sim.host("leader", move || node(leader.clone(), &FOLLOWERS));
    for (name, store) in FOLLOWERS.iter().zip(&stores[1..]) {
        let store = store.clone();
        sim.host(*name, move || node(store.clone(), &[]));
    }

    // the client sends its puts, then waits. the run ends when the client returns
    sim.client("client", async {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
        for val in 0..OPS {
            let key = KEYS[val as usize % KEYS.len()];
            socket.send_to(format!("PUT {key} {val}").as_bytes(), ("leader", PORT)).await?;
        }
        // 1 second of FAKE time. every message is delivered or lost before then
        // it takes almost no real time
        tokio::time::sleep(Duration::from_secs(1)).await;
        Ok(())
    });

    sim.run().unwrap();
    stores.iter().map(|s| s.borrow().clone()).collect()
}

/// rule 3 from milestone 4: at the end all stores are the same
fn all_agree(stores: &[BTreeMap<String, u64>]) -> bool {
    stores.iter().all(|s| *s == stores[0])
}

// ---------- the tests ----------

#[test]
fn perfect_network_all_agree() {
    // the client writes a=0,3,6,9  b=1,4,7  c=2,5,8. the last write of each key wins
    let expected = BTreeMap::from([("a".to_string(), 9), ("b".to_string(), 7), ("c".to_string(), 8)]);
    for seed in 0..20 {
        let stores = run(seed, &PERFECT);
        assert!(all_agree(&stores), "seed {seed}: {stores:?}");
        assert_eq!(stores[0], expected, "seed {seed}");
    }
}

#[test]
fn same_seed_gives_same_result() {
    for seed in 0..20 {
        assert_eq!(run(seed, &BAD), run(seed, &BAD), "seed {seed}");
    }
}

#[test]
fn hunt_finds_a_failing_seed() {
    let failing: Vec<u64> = (0..200).filter(|&seed| !all_agree(&run(seed, &BAD))).collect();
    println!("{} of 200 seeds failed. first failing seeds: {:?}", failing.len(), &failing[..failing.len().min(10)]);

    let seed = failing[0];
    let stores = run(seed, &BAD);
    println!("seed {seed}: leader={:?} f1={:?} f2={:?}", stores[0], stores[1], stores[2]);
    // replay: the same seed gives the same broken stores
    assert_eq!(run(seed, &BAD), stores);
}
