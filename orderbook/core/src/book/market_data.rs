//! Market data: queries that borrow the book.
//!
//! All of these take &self, so any number can run at once and none can
//! change the book.

use super::{Book, Depth, Level};
use crate::order::{Resting, Side};
use crate::types::{OrderId, Price};

impl Book {
    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next_back().copied()
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
    }

    /// `?` works on Option too: if either side is empty, return None.
    pub fn spread(&self) -> Option<i64> {
        Some(self.best_ask()? - self.best_bid()?)
    }

    /// One side's levels, best price first.
    ///
    /// `pub(super)`: visible in the parent module (`book`) and therefore in
    /// its other children too, like `submit`, which needs it for the FOK
    /// check. Without it, a method defined in this file would only be
    /// callable from this file. It can't be fully `pub`, because Level is
    /// private to `book`.
    ///
    /// The two arms produce DIFFERENT iterator types (Rev<Iter<..>> and
    /// Iter<..>), and a function returns ONE type, so `impl Iterator` can't
    /// cover both. Boxing them behind `dyn Iterator` can, at the cost of a
    /// heap allocation and dynamic calls. The `'_` says the iterator borrows
    /// from `self`, so the book can't change while it lives.
    pub(super) fn levels(&self, side: Side) -> Box<dyn Iterator<Item = (&Price, &Level)> + '_> {
        match side {
            Side::Buy => Box::new(self.bids.iter().rev()),
            Side::Sell => Box::new(self.asks.iter()),
        }
    }

    /// The top `n` levels of one side: what an exchange publishes as
    /// "market by price". A Vec, because market data is a snapshot that
    /// outlives the borrow of the book.
    pub fn depth(&self, side: Side, n: usize) -> Depth {
        self.levels(side)
            .take(n)
            .map(|(&price, level)| (price, level.total))
            .collect()
    }

    /// A borrowed view of one resting order. The returned reference can't
    /// outlive the book (lifetime elision ties it to &self).
    pub fn order(&self, id: OrderId) -> Option<&Resting> {
        let &(side, price) = self.index.get(&id)?;
        self.side(side)
            .get(&price)?
            .orders
            .iter()
            .find(|r| r.id == id)
    }

    /// Number of resting orders.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// No resting orders on either side.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}
