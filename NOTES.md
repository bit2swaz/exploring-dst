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