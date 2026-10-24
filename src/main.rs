mod check;
mod clock;
mod disk;
mod kv;
mod net;
mod rng;

use check::Checker;
use clock::Clock;
use kv::{CLIENT, LEADER, Msg, Node};
use net::{Faults, Network};
use rng::Rng;

const NODES: usize = 3;
const OPS: u64 = 10; // how many puts the client sends, by default
const KEYS: [&str; 3] = ["a", "b", "c"];

/// no drops, no duplicates, every message takes exactly 1 ms. thus no reorder
const PERFECT: Faults = Faults { drop_pct: 0, dup_pct: 0, max_delay_ms: 1 };
/// a bad network: drop, duplicate, delay and reorder. almost every seed fails
const BAD: Faults = Faults { drop_pct: 10, dup_pct: 10, max_delay_ms: 50 };
/// a lossy network: it keeps the order but loses 1% of the messages. ~5% of seeds fail
const LOSSY: Faults = Faults { drop_pct: 1, dup_pct: 0, max_delay_ms: 1 };

/// how many seeds `hunt` tries
const HUNT_SEEDS: u64 = 10_000;

/// everything about a run except the seed
#[derive(Clone, Copy)]
struct Setup {
    network: &'static str, // "perfect", "lossy" or "bad"
    planted_bug: bool,
    ops: u64, // how many puts the client sends
}

impl Setup {
    fn faults(&self) -> Faults {
        match self.network {
            "bad" => BAD,
            "lossy" => LOSSY,
            _ => PERFECT,
        }
    }

    /// the command line words that give this setup again
    fn words(&self) -> String {
        let bug = if self.planted_bug { " bug" } else { "" };
        format!("{}{bug} ops={}", self.network, self.ops)
    }
}

/// a fingerprint of the whole log (FNV-1a hash, written from scratch)
/// one changed character gives a diff number. so two logs with the same
/// fingerprint are the same log and you do not have to compare them line by line
fn fingerprint(log: &[String]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in log.join("\n").bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

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
fn simulate(seed: u64, setup: Setup) -> Outcome {
    let mut rng = Rng::new(seed);
    let mut clock = Clock::new();
    let mut net = Network::new(setup.faults());
    let mut log = Vec::new();
    let mut checker = Checker::new(NODES);
    let mut violation = None;
    // only the followers get the bug. the leader is always correct
    let mut nodes: Vec<Node> = (0..NODES).map(|id| Node::new(id, setup.planted_bug && id != LEADER)).collect();

    // the client sends all its puts at t=0. the rng picks the key. the value is the op number
    for val in 0..setup.ops {
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
fn find_failures(setup: Setup, n: u64) -> Vec<(u64, String)> {
    (0..n)
        .filter_map(|seed| simulate(seed, setup).violation.map(|e| (seed, e)))
        .collect()
}

/// the rule part of a violation, for example "rule 3"
fn rule_of(violation: &str) -> &str {
    violation.split(':').next().unwrap()
}

/// shrink: find the shortest run (fewest puts) that breaks the same rule
/// try 1 put on every seed, then 2 puts and so on. the first hit is the shortest
// note: brute force search. real shrinkers (proptest, hypothesis) cut down the
// random choices of the failing run itself. but this is enough for runs this small
fn shrink(setup: Setup, rule: &str) -> Option<(u64, Setup)> {
    for ops in 1..setup.ops {
        let small = Setup { ops, ..setup };
        let hit = find_failures(small, HUNT_SEEDS).into_iter().find(|(_, e)| rule_of(e) == rule);
        if let Some((seed, _)) = hit {
            return Some((seed, small));
        }
    }
    None
}

/// usage:
///   cargo run -- <seed> [bad|lossy] [bug] [ops=N]   run one seed and print its log
///   cargo run -- hunt [bad|lossy] [bug] [ops=N]     run many seeds, then shrink the failure
///   cargo run -- disk [<seed>] [bug]                the fake disk (milestone 8)
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |word: &str| args.iter().any(|a| a == word);
    let ops = args.iter().find_map(|a| a.strip_prefix("ops=")).map(|n| n.parse().expect("ops must be a number"));
    let setup = Setup {
        network: if has("bad") { "bad" } else if has("lossy") { "lossy" } else { "perfect" },
        planted_bug: has("bug"),
        ops: ops.unwrap_or(OPS),
    };

    if has("disk") {
        // milestone 8: the fake disk. a number = replay that seed, no number = hunt
        disk::cli(args.iter().find_map(|a| a.parse().ok()), setup.planted_bug);
    } else if has("hunt") {
        hunt(setup);
    } else {
        // the first argument is the seed. default = 42
        let seed: u64 = args.first().map(|s| s.parse().expect("seed must be a number")).unwrap_or(42);
        replay(seed, setup);
    }
}

/// print one failing run: its seed, its broken rule, its log size and fingerprint and how to replay it
fn report(title: &str, seed: u64, setup: Setup) {
    let out = simulate(seed, setup);
    println!("{title}: seed={seed}: {}", out.violation.unwrap());
    println!("  log: {} lines, fingerprint={:016x}", out.log.len(), fingerprint(&out.log));
    println!("  replay it: cargo run -- {seed} {}", setup.words());
}

fn hunt(setup: Setup) {
    println!("hunting over {HUNT_SEEDS} seeds [{}]", setup.words());
    let failures = find_failures(setup, HUNT_SEEDS);
    println!("{} of {HUNT_SEEDS} seeds failed", failures.len());

    if let Some((seed, e)) = failures.first() {
        let seeds: Vec<u64> = failures.iter().take(10).map(|(s, _)| *s).collect();
        println!("first failing seeds: {seeds:?}");
        report("first failure", *seed, setup);

        let rule = rule_of(e);
        println!("shrinking: looking for the shortest run that breaks {rule} ...");
        match shrink(setup, rule) {
            Some((small_seed, small)) => report("shortest failure", small_seed, small),
            None => println!("  no shorter run found"),
        }
        std::process::exit(1);
    }
}

fn replay(seed: u64, setup: Setup) {
    println!("seed={seed} [{}]", setup.words());
    let out = simulate(seed, setup);
    for line in &out.log {
        println!("{line}");
    }
    println!("log: {} lines, fingerprint={:016x}", out.log.len(), fingerprint(&out.log));
    if out.violation.is_some() {
        std::process::exit(1); // a broken rule = a failed run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERFECT_SETUP: Setup = Setup { network: "perfect", planted_bug: false, ops: OPS };
    const BAD_SETUP: Setup = Setup { network: "bad", ..PERFECT_SETUP };
    const LOSSY_SETUP: Setup = Setup { network: "lossy", ..PERFECT_SETUP };

    #[test]
    fn same_seed_gives_same_log() {
        assert_eq!(simulate(7, BAD_SETUP).log, simulate(7, BAD_SETUP).log);
    }

    #[test]
    fn different_seed_gives_different_log() {
        assert_ne!(simulate(7, BAD_SETUP).log, simulate(8, BAD_SETUP).log);
    }

    #[test]
    fn perfect_network_breaks_no_rule() {
        // 100 seeds = 100 diff random clients. no rule may break
        for seed in 0..100 {
            let out = simulate(seed, PERFECT_SETUP);
            assert_eq!(out.violation, None, "seed {seed}");
        }
    }

    #[test]
    fn planted_bug_gets_caught() {
        let out = simulate(42, Setup { planted_bug: true, ..PERFECT_SETUP });
        assert!(out.violation.unwrap().starts_with("rule 3"));
    }

    #[test]
    fn hunter_finds_the_rare_lossy_failure() {
        // seeds 0..15 pass. seed 15 is the first one that loses a message and breaks rule 3
        // change the rng or the order of rng calls and this number changes. that is on purpose:
        // a change in the "story" of a seed must be visible
        let failures = find_failures(LOSSY_SETUP, 100);
        assert_eq!(failures[0].0, 15);
        assert!(failures[0].1.starts_with("rule 3"));
    }

    #[test]
    fn bad_network_gets_caught() {
        // the naive design from milestone 3 has no version numbers so reorder breaks it
        let out = simulate(42, BAD_SETUP);
        assert!(out.violation.unwrap().starts_with("rule 2"));
    }

    #[test]
    fn replay_gives_the_same_fingerprint() {
        // the hunter and the replay call `simulate` at diff times. same seed = same log
        let first = find_failures(LOSSY_SETUP, 100)[0].0;
        assert_eq!(fingerprint(&simulate(first, LOSSY_SETUP).log), fingerprint(&simulate(first, LOSSY_SETUP).log));
        assert_ne!(fingerprint(&simulate(first, LOSSY_SETUP).log), fingerprint(&simulate(first + 1, LOSSY_SETUP).log));
    }

    #[test]
    fn shrink_finds_a_shorter_run_with_the_same_rule() {
        let long = simulate(15, LOSSY_SETUP);
        let (seed, small) = shrink(LOSSY_SETUP, "rule 3").unwrap();
        let short = simulate(seed, small);
        assert!(small.ops < LOSSY_SETUP.ops);
        assert!(short.log.len() < long.log.len());
        assert_eq!(rule_of(&short.violation.unwrap()), "rule 3");
    }
}
