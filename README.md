# exploring-dst

wanted to explore smth new for the weekend and since im a huge distributed systems guy (or at least consider myself to be), i picked up deterministic simulation testing (DST) :)

## what is DST

DST runs your system in a simulated world. the simulator controls the clock, the network, the disk and the random numbers (basically, anything that is non-deterministic, which is smth that even w the same input could give a diff output). all the random choices come from one number, i.e. the seed. the same seed gives the same run every time. thus when a test finds a bug, you can replay the bug with its seed

## run it

```sh
cargo run            # seed 42
cargo run -- 1234    # a different seed
cargo test           # determinism tests
```

## milestones

1. [x] seeded RNG and fake clock
2. [ ] fake network: drop, delay, duplicate, reorder
3. [ ] toy system: 3 nodes that replicate a small key-value store
4. [ ] invariants checked after every step
5. [ ] run thousands of seeds
6. [ ] replay and event log, then shrink the failing run
7. [ ] real code with `turmoil`
8. [ ] (bonus) fake disk: lost writes, torn writes, missing fsync

no external crates for milestones 1-6.
