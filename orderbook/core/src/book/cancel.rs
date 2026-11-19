//! Cancel and amend.
//!
//! Cancel REMOVES the order from the book and gives it back to the caller by
//! value: ownership leaves the book. Amend changes it in place.
//!
//! Amend can only REDUCE quantity. A smaller order keeps its place in the
//! queue, since that only hurts the trader who amended. Increasing quantity or
//! changing price would let an order jump ahead of everyone who queued after
//! it, so venues treat that as cancel + new order, which goes to the BACK.

use super::Book;
use crate::event::{CancelReason, Event, EventSink, Reject};
use crate::order::Resting;
use crate::types::{OrderId, Qty};

impl Book {
    /// Remove a resting order and return it.
    pub fn cancel(&mut self, id: OrderId, sink: &mut impl EventSink) -> Result<Resting, Reject> {
        let (side, price) = self.index.remove(&id).ok_or(Reject::UnknownOrder(id))?;
        let levels = self.side_mut(side);
        // If the index says an order is there and it isn't, the book is
        // corrupt. That's a bug, not bad input, so panic (phase 7).
        let level = levels
            .get_mut(&price)
            .expect("index points at a missing level");
        // O(orders at this price). Phase 9's slab makes this O(1).
        let pos = level
            .orders
            .iter()
            .position(|r| r.id == id)
            .expect("index points at a missing order");
        let order = level
            .orders
            .remove(pos)
            .expect("position came from this queue");
        level.total -= order.qty;
        if level.orders.is_empty() {
            levels.remove(&price);
        }
        sink.emit(Event::Cancelled {
            id,
            qty: order.qty,
            reason: CancelReason::Requested,
        });
        debug_assert_eq!(self.check(), Ok(()));
        Ok(order)
    }

    /// Reduce an order's open quantity to `qty`, keeping its time priority.
    pub fn amend(
        &mut self,
        id: OrderId,
        qty: Qty,
        sink: &mut impl EventSink,
    ) -> Result<(), Reject> {
        if qty == Qty::ZERO {
            return Err(Reject::ZeroQty); // that's a cancel
        }
        let &(side, price) = self.index.get(&id).ok_or(Reject::UnknownOrder(id))?;
        let level = self
            .side_mut(side)
            .get_mut(&price)
            .expect("index points at a missing level");
        let order = level
            .orders
            .iter_mut()
            .find(|r| r.id == id)
            .expect("index points at a missing order");
        if qty >= order.qty {
            return Err(Reject::AmendMustReduce {
                id,
                open: order.qty,
                requested: qty,
            });
        }
        let cut = order.qty - qty;
        order.qty = qty;
        level.total -= cut;
        sink.emit(Event::Amended { id, qty });
        debug_assert_eq!(self.check(), Ok(()));
        Ok(())
    }
}
