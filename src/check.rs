// invariants: rules that must be true during the whole simulation
//
// the simulator calls `after_step` after every delivered message
// when the network is empty it calls `at_end`
// at the first broken rule, the run stops

use crate::kv::{LEADER, Node};
use std::collections::{BTreeMap, BTreeSet};

pub struct Checker {
    seen_by_leader: BTreeSet<(String, u64)>, // every (key, value) that the leader ever had
    last: Vec<BTreeMap<String, u64>>,        // each store after the previous step
}

impl Checker {
    pub fn new(node_count: usize) -> Checker {
        Checker { seen_by_leader: BTreeSet::new(), last: vec![BTreeMap::new(); node_count] }
    }

    /// rules 1 and 2. they must be true after every step
    pub fn after_step(&mut self, nodes: &[Node]) -> Result<(), String> {
        for (key, &val) in &nodes[LEADER].store {
            self.seen_by_leader.insert((key.clone(), val));
        }

        for node in nodes {
            for (key, &val) in &node.store {
                // rule 1: no made up values. a node can only have a value that the leader had
                if !self.seen_by_leader.contains(&(key.clone(), val)) {
                    return Err(format!("rule 1: node{} has {key}={val}, but the leader never had it", node.id));
                }
                // rule 2: no going back. a key never changes to an older value
                // note: the value is the op number so bigger = newer. a real store needs a version number
                if let Some(&old) = self.last[node.id].get(key) {
                    if val < old {
                        return Err(format!("rule 2: node{} {key} went back from {old} to {val}", node.id));
                    }
                }
            }
            self.last[node.id] = node.store.clone();
        }
        Ok(())
    }

    /// rule 3. only true when no messages are left: all stores are the same
    pub fn at_end(&self, nodes: &[Node]) -> Result<(), String> {
        for node in nodes {
            if node.store != nodes[LEADER].store {
                return Err(format!(
                    "rule 3: node{} has {:?}, but the leader has {:?}",
                    node.id, node.store, nodes[LEADER].store
                ));
            }
        }
        Ok(())
    }
}
