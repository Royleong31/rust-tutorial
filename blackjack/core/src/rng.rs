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

#[cfg(test)]
mod tests {
    use super::*;

    fn draws(seed: &str, n: usize) -> Vec<u64> {
        let mut rng = Rng::new(seed.to_string(), 0, 51);
        (0..n).map(|_| rng.get_random_number()).collect()
    }

    #[test]
    fn same_seed_same_sequence() {
        assert_eq!(draws("abc", 100), draws("abc", 100));
    }

    #[test]
    fn different_seeds_differ() {
        assert_ne!(draws("abc", 100), draws("abd", 100));
    }

    #[test]
    fn stays_in_range_and_hits_both_ends() {
        let mut rng = Rng::new("range".to_string(), 3, 7);
        let values: Vec<u64> = (0..1_000).map(|_| rng.get_random_number()).collect();

        assert!(values.iter().all(|v| (3..=7).contains(v)));
        assert!(values.contains(&3), "min is inclusive");
        assert!(values.contains(&7), "max is inclusive");
    }

    #[test]
    #[should_panic]
    fn min_must_be_below_max() {
        Rng::new("x".to_string(), 5, 5);
    }

    #[test]
    fn debug_hides_seed() {
        let rng = Rng::new("super-secret-seed".to_string(), 0, 51);
        let debug = format!("{rng:?}");

        assert!(!debug.contains("super-secret-seed"), "{debug}");
        assert!(debug.contains("counter"), "{debug}");
    }
}
