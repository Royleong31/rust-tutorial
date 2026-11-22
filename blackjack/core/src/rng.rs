use std::fmt;

use sha2::{Digest, Sha256};

pub struct Rng {
    seed: String,
    counter: u64,
    min: u64,
    max: u64,
}

// Hand-written so the seed is never printed: seed + counter predicts every future card.
impl fmt::Debug for Rng {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rng")
            .field("counter", &self.counter)
            .field("min", &self.min)
            .field("max", &self.max)
            .finish_non_exhaustive()
    }
}

impl Rng {
    pub fn new(seed: String, min: u64, max: u64) -> Rng {
        assert!(min < max);

        Rng {
            seed: seed,
            counter: 0,
            min: min,
            max: max,
        }
    }

    fn next_u64(&mut self) -> u64 {
        let hash = Sha256::digest(format!("{}:{}", self.seed, self.counter));
        self.counter += 1;

        u64::from_be_bytes(hash[..8].try_into().unwrap())
    }

    pub fn get_random_number(&mut self) -> u64 {
        let range = self.max - self.min + 1;
        let limit = u64::MAX - (u64::MAX % range);

        loop {
            let new_random_u64 = self.next_u64();

            if new_random_u64 < limit {
                return new_random_u64 % range + self.min;
            }
        }
    }
}
