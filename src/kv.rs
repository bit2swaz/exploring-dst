// a toy replicated key-value store
//
// 3 nodes. node 0 is the leader, nodes 1 and 2 are followers.
// - a client sends `Put` to the leader
// - the leader saves it, then sends `Replicate` to each follower
// - a follower saves it
//
// naive on purpose: no order numbers, no retries, no replies.
// it only works when the network is perfect.
//
// a node never touches the network or the clock. it gets one message and returns
// the messages it wants to send. the simulator does the rest. this keeps nodes easy to test.

use std::collections::BTreeMap;

pub const LEADER: usize = 0;
pub const CLIENT: usize = 99; // a pretend id for the client. the client is not a node

#[derive(Clone, Debug)]
pub enum Msg {
    Put { key: String, val: u64 },       // client -> leader
    Replicate { key: String, val: u64 }, // leader -> follower
}

pub struct Node {
    pub id: usize,
    pub store: BTreeMap<String, u64>, // BTreeMap, so that printing it gives a fixed order
}

impl Node {
    pub fn new(id: usize) -> Node {
        Node { id, store: BTreeMap::new() }
    }

    /// handle one message. returns the messages to send, as (to, msg)
    pub fn on_message(&mut self, msg: Msg, node_count: usize) -> Vec<(usize, Msg)> {
        match msg {
            Msg::Put { key, val } => {
                self.store.insert(key.clone(), val);
                // tell every other node
                (0..node_count)
                    .filter(|&peer| peer != self.id)
                    .map(|peer| (peer, Msg::Replicate { key: key.clone(), val }))
                    .collect()
            }
            Msg::Replicate { key, val } => {
                self.store.insert(key, val);
                vec![]
            }
        }
    }
}
