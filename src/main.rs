mod check;
mod clock;
mod kv;
mod net;
mod rng;

use check::Checker;
use clock::Clock;
use kv::{CLIENT, LEADER, Msg, Node};
use net::{Faults, Network};
use rng::Rng;

const NODES: usize = 3;
const OPS: u64 = 10; // how many puts the client sends
const KEYS: [&str; 3] = ["a", "b", "c"];

/// no drops, no duplicates, every message takes exactly 1 ms. thus no reorder
const PERFECT: Faults = Faults { drop_pct: 0, dup_pct: 0, max_delay_ms: 1 };
/// a bad network: drop, duplicate, delay and reorder. almost every seed fails
const BAD: Faults = Faults { drop_pct: 10, dup_pct: 10, max_delay_ms: 50 };
/// a lossy network: it keeps the order but loses 1% of the messages. ~5% of seeds fail
const LOSSY: Faults = Faults { drop_pct: 1, dup_pct: 0, max_delay_ms: 1 };

/// how many seeds `hunt` tries
const HUNT_SEEDS: u64 = 10_000;

/// send one message and write it to the log
fn send(net: &mut Network<Msg>, rng: &mut Rng, clock: &Clock, log: &mut Vec<String>, from: usize, to: usize, msg: Msg) {
    log.push(format!("t={}ms send {msg:?} {from}->{to}", clock.now()));
    let copies = net.send(rng, clock.now(), from, to, msg);
    if copies != 1 {
        log.push(format!("  copies={copies}"));
    }
}

/// what one run gives back
struct Outcome {
    log: Vec<String>,
    violation: Option<String>, // the first broken rule or None if all rules held
}

/// run the key-value store simulation
/// `planted_bug` turns on the bug in the followers
fn simulate(seed: u64, faults: Faults, planted_bug: bool) -> Outcome {
    let mut rng = Rng::new(seed);
    let mut clock = Clock::new();
    let mut net = Network::new(faults);
    let mut log = Vec::new();
    let mut checker = Checker::new(NODES);
    let mut violation = None;
    // only the followers get the bug. the leader is always correct
    let mut nodes: Vec<Node> = (0..NODES).map(|id| Node::new(id, planted_bug && id != LEADER)).collect();

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
        // check the rules after every step. stop at the first broken rule
        if let Err(e) = checker.after_step(&nodes) {
            violation = Some(e);
            break;
        }
    }
    // the network is empty (or we stopped). if no rule broke yet then check rule 3
    if violation.is_none() {
        violation = checker.at_end(&nodes).err();
    }

    for node in &nodes {
        log.push(format!("end node{} {:?}", node.id, node.store));
    }
    if let Some(e) = &violation {
        log.push(format!("VIOLATION at t={}ms: {e}", clock.now()));
    }
    Outcome { log, violation }
}

/// run seeds 0..n and return each seed that broke a rule w the broken rule
fn find_failures(faults: Faults, planted_bug: bool, n: u64) -> Vec<(u64, String)> {
    (0..n)
        .filter_map(|seed| simulate(seed, faults, planted_bug).violation.map(|e| (seed, e)))
        .collect()
}

/// usage:
///   cargo run -- <seed> [bad|lossy] [bug]     run one seed and print its log
///   cargo run -- hunt [bad|lossy] [bug]       run many seeds and print the failing ones
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // extra words: "bad" or "lossy" = that network (default: perfect), "bug" = planted bug
    let has = |word: &str| args.iter().any(|a| a == word);
    let faults = if has("bad") { BAD } else if has("lossy") { LOSSY } else { PERFECT };
    let planted_bug = has("bug");

    if has("hunt") {
        hunt(faults, planted_bug, &args[1..].join(" "));
    } else {
        // the first argument is the seed. default = 42
        let seed: u64 = args.first().map(|s| s.parse().expect("seed must be a number")).unwrap_or(42);
        replay(seed, faults, planted_bug);
    }
}

fn hunt(faults: Faults, planted_bug: bool, words: &str) {
    println!("hunting over {HUNT_SEEDS} seeds [{words}]");
    let failures = find_failures(faults, planted_bug, HUNT_SEEDS);
    println!("{} of {HUNT_SEEDS} seeds failed", failures.len());

    if let Some((seed, e)) = failures.first() {
        let seeds: Vec<u64> = failures.iter().take(10).map(|(s, _)| *s).collect();
        println!("first failing seeds: {seeds:?}");
        println!("first failure: seed={seed}: {e}");
        println!("replay it: cargo run -- {seed} {words}");
        std::process::exit(1);
    }
}

fn replay(seed: u64, faults: Faults, planted_bug: bool) {
    println!("seed={seed}");
    let out = simulate(seed, faults, planted_bug);
    for line in &out.log {
        println!("{line}");
    }
    if out.violation.is_some() {
        std::process::exit(1); // a broken rule = a failed run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_log() {
        assert_eq!(simulate(7, BAD, false).log, simulate(7, BAD, false).log);
    }

    #[test]
    fn different_seed_gives_different_log() {
        assert_ne!(simulate(7, BAD, false).log, simulate(8, BAD, false).log);
    }

    #[test]
    fn perfect_network_breaks_no_rule() {
        // 100 seeds = 100 different random clients. no rule may break
        for seed in 0..100 {
            let out = simulate(seed, PERFECT, false);
            assert_eq!(out.violation, None, "seed {seed}");
        }
    }

    #[test]
    fn planted_bug_gets_caught() {
        let out = simulate(42, PERFECT, true);
        assert!(out.violation.unwrap().starts_with("rule 3"));
    }

    #[test]
    fn hunter_finds_the_rare_lossy_failure() {
        // seeds 0..15 pass. seed 15 is the first one that loses a message and breaks rule 3
        // change the rng or the order of rng calls and this number changes. that is on purpose:
        // a change in the "story" of a seed must be visible
        let failures = find_failures(LOSSY, false, 100);
        assert_eq!(failures[0].0, 15);
        assert!(failures[0].1.starts_with("rule 3"));
    }

    #[test]
    fn bad_network_gets_caught() {
        // the naive design from milestone 3 has no version numbers so reorder breaks it
        let out = simulate(42, BAD, false);
        assert!(out.violation.unwrap().starts_with("rule 2"));
    }
}
