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

## milestone 7: real code w turmoil

- wrote a tiny real async service (leader + 2 followers over UDP) w same naive design as milestone 3. turmoil runs all 3 hosts on 1 thread w a fake clock, a fake network and one seed
- the service code doesnt know its in a simulation. the only change is the import: `turmoil::net::UdpSocket` in place of `tokio::net::UdpSocket`
- used UDP and not TCP: turmoil never re-sends a lost TCP segment so TCP faults there do not act like real TCP
- bad network (5% link breaks, 1 - 100ms latency): 147 of 200 seeds fail. seed 0: leader has a=6 but f1 has a=9. the follower is "more right" than the leader because both got the puts out of order
- mild network (1% link breaks, no reorder): 12 of 200 fail. seed 26: f2 missed every replicate while its link was broken. same bug as our milestone 5 lossy run

## milestone 8 (bonus): fake disk

- fake disk with 2 layers: the page cache (`write`) and the durable disk (`fsync`). on a power cut, each write that is not fsync'd is lost, torn or kept. the seed picks when the power goes out
- a tiny WAL on top: 10 byte records with a 1 byte checksum. after the crash, recovery reads records until the first bad checksum
- rule A: every put that said "ok" survives. rule B: every recovered record is the one we wrote
- correct log (fsync before "ok"): 0 of 10k seeds fail even w torn writes. planted bug (no fsync): 9296 of 10k fail. with no crash the bug can never show
- swapped the checksum for an "all zeros = end of log" check: 151 of 10k fail with garbage records. and on seed 7 a torn record came back correct by pure luck