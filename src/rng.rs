// a seeded random number generator (rng), written from scratch
//
// algo is splitmix64: small and fast and its output looks random
// it is NOT safe for passwords or keys, but for a simulation, its good enough
//
// imp: the RNG has no hidden input. seed is the only input
// same seed = same numbers, same order, on every run

pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// give the next random number (any u64 value)
    pub fn next_u64(&mut self) -> u64 {
        // step the state by a fixed odd number
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        // mix the bits so that near states give very diff outputs
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// give a random number from 0 to n-1
    // note: modulo has a very small bias for large n so its not imp for this simulation
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}
