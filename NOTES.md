# notes

## milestone 1: seeded rng and fake clock

- wrote a tiny rng (splitmix64) and a fake clock. the rng only takes a seed. the clock only moves when the simulator calls `advance`
- same seed gives the same output on every run (obv means diff seed = diff output)
