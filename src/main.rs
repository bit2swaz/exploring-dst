mod clock;
mod net;
mod rng;

use clock::Clock;
use net::{Faults, Network};
use rng::Rng;

/// the network faults for `cargo run`. change these to see a diff network
const FAULTS: Faults = Faults { drop_pct: 10, dup_pct: 10, max_delay_ms: 50 };

/// run a tiny simulation and return its event log
/// node 0 sends 10 messages (m0..m9) to node 1, in order, all at t=0
/// then the simulator delivers them one by one in order of arrival time
fn simulate(seed: u64, faults: Faults) -> Vec<String> {
    let mut rng = Rng::new(seed);
    let mut clock = Clock::new();
    let mut net = Network::new(faults);
    let mut log = Vec::new();

    for i in 0..10 {
        let copies = net.send(&mut rng, clock.now(), 0, 1, format!("m{i}"));
        log.push(format!("t={}ms send m{i} 0->1 copies={copies}", clock.now()));
    }

    // deliver until the network is empty. jump the clock to each arrival time
    while let Some((at, env)) = net.next() {
        clock.advance(at - clock.now());
        log.push(format!("t={}ms deliver {} {}->{}", clock.now(), env.msg, env.from, env.to));
    }
    log
}

fn main() {
    // seed from the first argument. default = 42
    let seed: u64 = std::env::args()
        .nth(1)
        .map(|s| s.parse().expect("seed must be a number"))
        .unwrap_or(42);

    println!("seed={seed}");
    for line in simulate(seed, FAULTS) {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the message names from the "deliver" lines in delivery order
    fn delivered(log: &[String]) -> Vec<String> {
        log.iter()
            .filter(|line| line.contains(" deliver "))
            .map(|line| line.split(' ').nth(2).unwrap().to_string())
            .collect()
    }

    #[test]
    fn same_seed_gives_same_log() {
        assert_eq!(simulate(7, FAULTS), simulate(7, FAULTS));
    }

    #[test]
    fn different_seed_gives_different_log() {
        assert_ne!(simulate(7, FAULTS), simulate(8, FAULTS));
    }

    #[test]
    fn perfect_network_keeps_order() {
        // no drops, no duplicates, every message takes exactly 1 ms
        let perfect = Faults { drop_pct: 0, dup_pct: 0, max_delay_ms: 1 };
        let expected: Vec<String> = (0..10).map(|i| format!("m{i}")).collect();
        assert_eq!(delivered(&simulate(42, perfect)), expected);
    }

    #[test]
    fn faulty_network_reorders() {
        let got = delivered(&simulate(42, FAULTS));
        let mut sorted = got.clone();
        sorted.sort();
        assert_ne!(got, sorted, "seed 42 must deliver out of order");
    }

    #[test]
    fn full_drop_delivers_nothing() {
        let black_hole = Faults { drop_pct: 100, ..FAULTS };
        assert!(delivered(&simulate(42, black_hole)).is_empty());
    }

    #[test]
    fn clock_moves_only_on_advance() {
        let mut clock = Clock::new();
        assert_eq!(clock.now(), 0);
        assert_eq!(clock.now(), 0); // reading the time doesnt move it
        clock.advance(25);
        assert_eq!(clock.now(), 25);
    }
}
