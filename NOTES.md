# notes

## milestone 1: seeded rng and fake clock

- wrote a tiny rng (splitmix64) and a fake clock. the rng only takes a seed. the clock only moves when the simulator calls `advance`
- same seed gives the same output on every run (obv means diff seed = diff output)

## milestone 2: fake network

- added a fake network. `send` puts a message on the wire with a random arrival time / drops it / puts 2 copies on it. the simulator delivers in order of arrival time
- reorder is free since random delays already shuffle the messages. no extra code needed ^^

## milestone 3: toy replicated kv store

- 3 nodes. a client sends puts to the leader (node 0) and the leader tells the 2 followers. no order numbers, no retries, no replies
- a node never touches the network or the clock. it gets 1 message and returns the messages it wants to send. the simulator does everything else
- on a perfect network, all 100 test seeds end with 3 equal stores

## milestone 4: invariants

- 3 new rules which are checked by the computer: no made up values, no going back (both after every step), and all stores are the same (only at the end when no messages are left)
- planted a bug on purpose: followers use `or_insert` instead of `insert` so they keep the first value forever. rule 3 catches it on every seed (as it should)
- the naive design from milestone 3 breaks rule 2 on all 100 bad network seeds. without the per step check, 7 of those 100 seeds pass because the stores agree at the end purely by luck

## milestone 5: hunt for failing seeds

- `cargo run -- hunt lossy` runs 10k seeds in ~1 second and prints the failing ones + the command to replay the first one
- the lossy network keeps the order but loses 1% of messages. only 555 of the 10k seeds fail. the first is seed 15: a `Replicate b=9` to node1 got lost so node1 is stuck on b=8
- a lost `Put` does no harm because since the leader never had it, nobody has it. only a lost `Replicate` breaks the rules

## milestone 6: replay and shrink

- every log gets a fingerprint (a fnv-1a hash). the hunter and the replay print the same fingerprint so the replay is the exact same story
- shrink = find the shortest run that breaks the same rule. try 1 put on every seed, then 2 and so on. the lossy failure went from 60 log lines (seed 15, 10 puts) to 10 lines (seed 117, 1 put)
- the shortest run tells you what the bug exactly needs, say for example, a lost replicate needs 1 put. going back in time (rule 2) and the planted `or_insert` bug both need 2 puts to the same key
- swapped BTreeMap for HashMap: seed 15 gave 5 diff fingerprints in 5 runs. rust's HashMap picks a new random order in each process. one HashMap and the replay is gone