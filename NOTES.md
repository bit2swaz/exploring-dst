# notes

## milestone 1: seeded rng and fake clock

- wrote a tiny rng (splitmix64) and a fake clock. the rng only takes a seed. the clock only moves when the simulator calls `advance`
- same seed gives the same output on every run (obv means diff seed = diff output)

## milestone 2: fake network

- added a fake network. `send` puts a message on the wire with a random arrival time / drops it / puts 2 copies on it. the simulator delivers in order of arrival time
- reorder is free since random delays already shuffle the messages. no extra code needed ^^
