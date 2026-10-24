// a fake network
//
// a message is not delivered at once. `send` puts it in a waiting list w an arrival time
// the simulator takes the messages out of the list in order of arrival time
//
// all the "bad luck" comes from the seeded rng:
// - drop: the message is lost
// - duplicate: the message arrives two times
// - delay: each copy gets a random arrival time
// - reorder: random delays reorder the messages (no special code)

use crate::rng::Rng;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub struct Faults {
    pub drop_pct: u64,     // chance to lose a message
    pub dup_pct: u64,      // chance to deliver a message two times
    pub max_delay_ms: u64, // each copy arrives 1..=max_delay_ms after the send
}

#[derive(Clone)]
pub struct Envelope<M> {
    pub from: usize,
    pub to: usize,
    pub msg: M,
}

pub struct Network<M> {
    faults: Faults,
    // the messages that are "on the wire" keyed by (arrival time, send number).
    // a BTreeMap keeps its keys sorted so the first entry always arrives first.
    // two messages can have the same arrival time. the send number breaks the tie
    // in a fixed way. (a HashMap has no fixed order so we do not use it)
    in_flight: BTreeMap<(u64, u64), Envelope<M>>,
    next_seq: u64,
}

impl<M: Clone> Network<M> {
    pub fn new(faults: Faults) -> Network<M> {
        Network { faults, in_flight: BTreeMap::new(), next_seq: 0 }
    }

    /// send a message. returns how many copies are on the wire: 0 (dropped), 1 or 2 (duplicated)
    pub fn send(&mut self, rng: &mut Rng, now: u64, from: usize, to: usize, msg: M) -> u32 {
        if rng.chance(self.faults.drop_pct) {
            return 0;
        }
        let env = Envelope { from, to, msg };
        self.put_on_wire(rng, now, env.clone());
        if rng.chance(self.faults.dup_pct) {
            self.put_on_wire(rng, now, env);
            return 2;
        }
        1
    }

    /// take out the message w the earliest arrival time w that time
    pub fn next(&mut self) -> Option<(u64, Envelope<M>)> {
        self.in_flight.pop_first().map(|((at, _seq), env)| (at, env))
    }

    fn put_on_wire(&mut self, rng: &mut Rng, now: u64, env: Envelope<M>) {
        let at = now + 1 + rng.below(self.faults.max_delay_ms);
        self.in_flight.insert((at, self.next_seq), env);
        self.next_seq += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAD: Faults = Faults { drop_pct: 10, dup_pct: 10, max_delay_ms: 50 };

    /// send messages 0..10 from node 0 to node 1, all at t=0. return them in delivery order
    fn deliver_all(seed: u64, faults: Faults) -> Vec<u64> {
        let mut rng = Rng::new(seed);
        let mut net = Network::new(faults);
        for i in 0..10 {
            net.send(&mut rng, 0, 0, 1, i);
        }
        let mut got = Vec::new();
        while let Some((_at, env)) = net.next() {
            got.push(env.msg);
        }
        got
    }

    #[test]
    fn perfect_network_keeps_order() {
        let perfect = Faults { drop_pct: 0, dup_pct: 0, max_delay_ms: 1 };
        assert_eq!(deliver_all(42, perfect), (0..10).collect::<Vec<u64>>());
    }

    #[test]
    fn bad_network_reorders() {
        let got = deliver_all(42, BAD);
        let mut sorted = got.clone();
        sorted.sort();
        assert_ne!(got, sorted, "seed 42 must deliver out of order");
    }

    #[test]
    fn full_drop_delivers_nothing() {
        assert!(deliver_all(42, Faults { drop_pct: 100, ..BAD }).is_empty());
    }
}
