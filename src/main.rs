mod clock;
mod kv;
mod net;
mod rng;

use clock::Clock;
use kv::{CLIENT, LEADER, Msg, Node};
use net::{Faults, Network};
use rng::Rng;

const NODES: usize = 3;
const OPS: u64 = 10; // how many puts the client sends
const KEYS: [&str; 3] = ["a", "b", "c"];

/// no drops, no duplicates, every message takes exactly 1 ms. thus no reorder
const PERFECT: Faults = Faults { drop_pct: 0, dup_pct: 0, max_delay_ms: 1 };
/// a bad network. use it with `cargo run -- <seed> bad`
const FAULTS: Faults = Faults { drop_pct: 10, dup_pct: 10, max_delay_ms: 50 };

/// send one message and write it to the log
fn send(net: &mut Network<Msg>, rng: &mut Rng, clock: &Clock, log: &mut Vec<String>, from: usize, to: usize, msg: Msg) {
    log.push(format!("t={}ms send {msg:?} {from}->{to}", clock.now()));
    let copies = net.send(rng, clock.now(), from, to, msg);
    if copies != 1 {
        log.push(format!("  copies={copies}"));
    }
}

/// run the key-value store simulation. returns the event log and the nodes at the end
fn simulate(seed: u64, faults: Faults) -> (Vec<String>, Vec<Node>) {
    let mut rng = Rng::new(seed);
    let mut clock = Clock::new();
    let mut net = Network::new(faults);
    let mut log = Vec::new();
    let mut nodes: Vec<Node> = (0..NODES).map(Node::new).collect();

    // the client sends all its puts at t=0. the rng picks the key. the value is the op number
    for val in 0..OPS {
        let key = KEYS[rng.below(KEYS.len() as u64) as usize].to_string();
        send(&mut net, &mut rng, &clock, &mut log, CLIENT, LEADER, Msg::Put { key, val });
    }

    // deliver until the network is empty. each delivered message can make new messages
    while let Some((at, env)) = net.next() {
        clock.advance(at - clock.now());
        log.push(format!("t={}ms deliver {:?} {}->{}", clock.now(), env.msg, env.from, env.to));
        for (to, msg) in nodes[env.to].on_message(env.msg, NODES) {
            send(&mut net, &mut rng, &clock, &mut log, env.to, to, msg);
        }
    }

    for node in &nodes {
        log.push(format!("end node{} {:?}", node.id, node.store));
    }
    (log, nodes)
}

fn main() {
    // seed from the first argument. default = 42
    let seed: u64 = std::env::args()
        .nth(1)
        .map(|s| s.parse().expect("seed must be a number"))
        .unwrap_or(42);
    // second argument "bad" turns on the bad network
    let faults = if std::env::args().nth(2).as_deref() == Some("bad") { FAULTS } else { PERFECT };

    println!("seed={seed}");
    let (log, _nodes) = simulate(seed, faults);
    for line in log {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_log() {
        assert_eq!(simulate(7, FAULTS).0, simulate(7, FAULTS).0);
    }

    #[test]
    fn different_seed_gives_different_log() {
        assert_ne!(simulate(7, FAULTS).0, simulate(8, FAULTS).0);
    }

    #[test]
    fn perfect_network_all_nodes_agree() {
        // 100 seeds = 100 different random clients. all must end with 3 equal stores
        for seed in 0..100 {
            let (_log, nodes) = simulate(seed, PERFECT);
            assert!(!nodes[LEADER].store.is_empty());
            for node in &nodes {
                assert_eq!(node.store, nodes[LEADER].store, "seed {seed}: node{} differs", node.id);
            }
        }
    }
}
