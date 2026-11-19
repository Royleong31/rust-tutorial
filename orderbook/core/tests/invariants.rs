//! Thousands of random operations; check() runs after each one (inside the
//! methods, through debug_assert!) and once more here.
//!   cargo test -p orderbook --test invariants

mod common;

use common::*;
use orderbook::{Book, NewOrder, OrderId, OrderType, Price, Qty, Side, TimeInForce, TraderId};

/// A tiny deterministic PRNG, so a failure always reproduces.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

#[test]
fn random_operations_keep_invariants() {
    let mut rng = Lcg(42);
    let mut b = Book::new(TICK);
    let mut ev = Vec::new();
    let mut sent = 0;
    // The book's id counter is private, so track the next id from what
    // submit returns. (The capstone's test read `b.next_id` directly.)
    let mut next_id = 1;
    for _ in 0..5_000 {
        match rng.below(10) {
            0..=5 => {
                let side = if rng.below(2) == 0 {
                    Side::Buy
                } else {
                    Side::Sell
                };
                let trader = TraderId(rng.below(4) as u32);
                let qty = Qty(1 + rng.below(10));
                let price = Price(950 + 5 * rng.below(21) as i64);
                let tif = [
                    TimeInForce::Gtc,
                    TimeInForce::Gtc,
                    TimeInForce::Ioc,
                    TimeInForce::Fok,
                    TimeInForce::PostOnly,
                ][rng.below(5) as usize];
                let kind = if rng.below(10) == 0 {
                    OrderType::Market
                } else {
                    OrderType::Limit { price, tif }
                };
                sent += qty.0;
                let id = b
                    .submit(
                        NewOrder {
                            trader,
                            side,
                            qty,
                            kind,
                        },
                        &mut ev,
                    )
                    .unwrap();
                next_id = id.0 + 1;
            }
            6..=8 => {
                let _ = b.cancel(OrderId(1 + rng.below(next_id)), &mut ev);
            }
            _ => {
                let _ = b.amend(
                    OrderId(1 + rng.below(next_id)),
                    Qty(1 + rng.below(5)),
                    &mut ev,
                );
            }
        }
        assert_eq!(b.check(), Ok(()));
    }
    let traded: u64 = trades(&ev).iter().map(|t| t.2).sum();
    assert!(traded > 0 && traded <= sent);
    assert!(!b.is_empty());
}
