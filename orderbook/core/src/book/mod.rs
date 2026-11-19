//! The book: its data, and helpers shared by its submodules.
//!
//! Phase 6's layout, plus a cached total per level:
//!
//!   bids:  BTreeMap<Price, Level>     sorted; best bid = LAST key
//!   asks:  BTreeMap<Price, Level>     sorted; best ask = FIRST key
//!   Level: VecDeque<Resting> + total  FIFO queue = time priority
//!   index: HashMap<OrderId, (Side, Price)>
//!                                     cancel/amend jump straight to the level
//!
//! The book OWNS every resting order. Nothing else holds a pointer to one, so
//! there's no Rc or RefCell anywhere: outside code refers to an order by its
//! OrderId and asks the book.
//!
//! `impl Book` is spread over several files, one per job. That works because
//! of how privacy works in Rust. A private item (no `pub`) is visible in the
//! module that defines it AND in all of that module's children. The struct
//! and its private fields are defined here in `book`, so `book::submit`,
//! `book::cancel` and the others can all reach `self.bids` directly. Code
//! outside `book` can't, and has to use the public methods.

mod cancel;
mod invariants;
mod market_data;
mod submit;

use crate::order::{Resting, Side};
use crate::types::{OrderId, Price, Qty};
use std::collections::btree_map::OccupiedEntry;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;

/// (price, total qty) per level, best first.
pub type Depth = Vec<(Price, Qty)>;

/// One price level. Private to `book`: nobody outside needs to know levels exist.
#[derive(Debug, Default)]
struct Level {
    orders: VecDeque<Resting>,
    /// Sum of `orders[..].qty`, kept up to date so depth needs no summing.
    /// A cache like this is a promise every method must keep (see invariants.rs).
    total: Qty,
}

/// A limit order book for one instrument.
#[derive(Debug)]
pub struct Book {
    tick: i64,
    bids: BTreeMap<Price, Level>,
    asks: BTreeMap<Price, Level>,
    index: HashMap<OrderId, (Side, Price)>,
    next_id: u64,
}

impl Book {
    /// An empty book. Prices must be positive multiples of `tick`.
    ///
    /// # Panics
    /// If `tick` isn't positive. That's a programmer error, not bad input.
    pub fn new(tick: i64) -> Book {
        assert!(tick > 0, "tick size must be positive");
        Book {
            tick,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            index: HashMap::new(),
            next_id: 1,
        }
    }

    /// The tick size this book was created with.
    pub fn tick(&self) -> i64 {
        self.tick
    }

    // Private helpers. Usable here and in the child modules, invisible outside.
    fn side(&self, side: Side) -> &BTreeMap<Price, Level> {
        match side {
            Side::Buy => &self.bids,
            Side::Sell => &self.asks,
        }
    }

    fn side_mut(&mut self, side: Side) -> &mut BTreeMap<Price, Level> {
        match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        }
    }
}

/// The best level of `levels`, which holds `side`'s orders: the HIGHEST bid
/// or the LOWEST ask. An OccupiedEntry lets the caller change the level and
/// then delete it without looking it up again. Its lifetime ('_) ties it to
/// the &mut borrow of `levels`.
fn best_entry(
    levels: &mut BTreeMap<Price, Level>,
    side: Side,
) -> Option<OccupiedEntry<'_, Price, Level>> {
    match side {
        Side::Buy => levels.last_entry(),
        Side::Sell => levels.first_entry(),
    }
}

/// Can a taker on `side`, with this limit, trade at a level priced `level`?
fn crosses(side: Side, limit: Option<Price>, level: Price) -> bool {
    match (side, limit) {
        (_, None) => true, // market order: any price
        (Side::Buy, Some(limit)) => level <= limit,
        (Side::Sell, Some(limit)) => level >= limit,
    }
}

impl fmt::Display for Book {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn queue(level: &Level) -> String {
            level
                .orders
                .iter()
                .map(|r| format!("{}:{}", r.id, r.qty))
                .collect::<Vec<_>>()
                .join(" ")
        }
        // Asks high to low, so the best ask sits just above the spread line.
        for (price, level) in self.asks.iter().rev() {
            writeln!(
                f,
                "    ASK {price:>6} {:>5}   {}",
                level.total,
                queue(level)
            )?;
        }
        match self.spread() {
            Some(s) => writeln!(f, "    ────────── spread {s} ──────────")?,
            None => writeln!(f, "    ────────── no spread ──────────")?,
        }
        for (price, level) in self.bids.iter().rev() {
            writeln!(
                f,
                "    BID {price:>6} {:>5}   {}",
                level.total,
                queue(level)
            )?;
        }
        Ok(())
    }
}

// UNIT TESTS live inside the crate, so they can see private items: the
// fields, Level, the helpers. Use them for things the public API can't
// show. Behaviour that users rely on is tested from the OUTSIDE, in the
// integration tests under tests/.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{NewOrder, TimeInForce};
    use crate::types::TraderId;

    fn post(book: &mut Book, trader: u32, qty: u64, price: i64) {
        let o = NewOrder::limit(
            TraderId(trader),
            Side::Buy,
            Qty(qty),
            Price(price),
            TimeInForce::PostOnly,
        );
        book.submit(o, &mut Vec::new()).unwrap();
    }

    #[test]
    fn post_only_joins_the_back_of_the_queue() {
        let mut b = Book::new(5);
        post(&mut b, 1, 6, 1000);
        post(&mut b, 2, 1, 1000);
        let queue: Vec<u64> = b.bids[&Price(1000)].orders.iter().map(|r| r.id.0).collect();
        assert_eq!(queue, vec![1, 2]);
    }

    #[test]
    fn check_catches_a_stale_total() {
        let mut b = Book::new(5);
        post(&mut b, 1, 6, 1000);
        // Corrupt the cache by hand. Only code inside the crate can do this,
        // which is exactly why the fields are private.
        b.bids.get_mut(&Price(1000)).unwrap().total = Qty(99);
        assert!(b.check().unwrap_err().contains("total says 99"));
    }
}
