mod clock;
mod rng;

use clock::Clock;
use rng::Rng;

/// run a tiny simulation and return its event log
/// on each step: the RNG picks a wait time, the clock moves and a "dice roll" happens
fn simulate(seed: u64, steps: u32) -> Vec<String> {
    let mut rng = Rng::new(seed);
    let mut clock = Clock::new();
    let mut log = Vec::new();

    for step in 0..steps {
        let wait = 1 + rng.below(100); // wait 1..=100 ms
        clock.advance(wait);
        let roll = 1 + rng.below(6); // dice: 1..=6
        log.push(format!("step={step} t={}ms roll={roll}", clock.now()));
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
    for line in simulate(seed, 5) {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_log() {
        assert_eq!(simulate(7, 100), simulate(7, 100));
    }

    #[test]
    fn different_seed_gives_different_log() {
        assert_ne!(simulate(7, 100), simulate(8, 100));
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
