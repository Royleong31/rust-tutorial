//! Invariants: what must ALWAYS be true.
//!
//! The book keeps the same facts in several places (a level's queue and its
//! total; the levels and the index). Each method has to update all of them,
//! and forgetting one is the classic order book bug. So every mutating method
//! ends with debug_assert_eq!(self.check(), Ok(())):
//!   - on in `cargo run` and `cargo test`, so a bug is caught at the first
//!     operation that causes it, not three hundred operations later
//!   - compiled out of `--release` builds, because it scans the whole book
//!
//! The randomized integration test runs thousands of operations through it.

use super::Book;
use crate::order::Side;
use crate::types::Qty;

impl Book {
    /// Check every invariant. Err describes the first one that's broken.
    /// It's `pub` so tests (and a paranoid caller) can run it too.
    pub fn check(&self) -> Result<(), String> {
        for (side, levels) in [(Side::Buy, &self.bids), (Side::Sell, &self.asks)] {
            for (&price, level) in levels {
                if level.orders.is_empty() {
                    return Err(format!("empty {side} level at {price}"));
                }
                let sum: Qty = level.orders.iter().map(|r| r.qty).sum();
                if sum != level.total {
                    return Err(format!(
                        "{side} {price}: total says {} but orders add up to {sum}",
                        level.total
                    ));
                }
                for r in &level.orders {
                    if r.qty == Qty::ZERO {
                        return Err(format!("{} rests with zero qty", r.id));
                    }
                    if self.index.get(&r.id) != Some(&(side, price)) {
                        return Err(format!(
                            "{} is at {side} {price} but the index disagrees",
                            r.id
                        ));
                    }
                }
            }
        }
        let resting: usize = self
            .bids
            .values()
            .chain(self.asks.values())
            .map(|l| l.orders.len())
            .sum();
        if resting != self.index.len() {
            return Err(format!(
                "{resting} orders rest but the index has {}",
                self.index.len()
            ));
        }
        // A "let chain" (edition 2024): `if let` and a condition, joined by &&.
        if let (Some(bid), Some(ask)) = (self.best_bid(), self.best_ask())
            && bid >= ask
        {
            return Err(format!("book is crossed: bid {bid} >= ask {ask}"));
        }
        Ok(())
    }
}
