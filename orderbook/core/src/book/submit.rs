//! Submitting: validate, match, then rest or cancel what's left.
//!
//! The order of the steps matters: everything that can fail is checked
//! BEFORE anything changes. A function that errors halfway through a
//! mutation leaves half-updated state behind, and in a book that means
//! orders that exist in the index but not in a level, or totals that lie.

use super::{Book, best_entry, crosses}; // private items of the parent module: visible to its children
use crate::event::{CancelReason, Event, EventSink, Reject};
use crate::order::{NewOrder, OrderType, Resting, Side, TimeInForce};
use crate::types::{OrderId, Price, Qty, TraderId};

impl Book {
    /// Validate, match and (depending on the time in force) rest an order.
    /// Returns the id it was given, or a Reject if it was invalid, in which
    /// case nothing changed and no events were emitted.
    pub fn submit(&mut self, o: NewOrder, sink: &mut impl EventSink) -> Result<OrderId, Reject> {
        // 1. Validate. `?` returns the Reject; nothing has changed yet.
        self.validate(&o)?;

        // 2. Accept. From here on the order exists, and every outcome is an
        //    event, never an Err.
        let id = OrderId(self.next_id);
        self.next_id += 1;
        sink.emit(Event::Accepted { id, order: o });

        // 3. Conditions that look at the book before any trading.
        if let OrderType::Limit { price, tif } = o.kind {
            let kill = match tif {
                TimeInForce::PostOnly if self.would_cross(o.side, price) => {
                    Some(CancelReason::WouldCross)
                }
                TimeInForce::Fok if self.fillable(&o) < o.qty => Some(CancelReason::FillOrKill),
                _ => None,
            };
            if let Some(reason) = kill {
                sink.emit(Event::Cancelled {
                    id,
                    qty: o.qty,
                    reason,
                });
                return Ok(id);
            }
        }

        // 4. Trade against the other side.
        let left = self.match_order(id, &o, sink);

        // 5. The remainder. No `_` arm on purpose: add a TimeInForce variant
        //    and this stops compiling until you decide what it does here.
        if left > Qty::ZERO {
            match o.kind {
                OrderType::Limit {
                    price,
                    tif: TimeInForce::Gtc | TimeInForce::PostOnly,
                } => self.rest(id, o.trader, o.side, price, left, sink),
                OrderType::Limit {
                    tif: TimeInForce::Ioc | TimeInForce::Fok,
                    ..
                }
                | OrderType::Market => sink.emit(Event::Cancelled {
                    id,
                    qty: left,
                    reason: CancelReason::Unfilled,
                }),
            }
        }

        debug_assert_eq!(self.check(), Ok(()));
        Ok(id)
    }

    // The methods below have no `pub`, and they're defined in `book::submit`,
    // so ONLY this file can call them. Nothing else needs to.

    /// "Parse, don't validate" (phase 7) would make a ValidOrder type that
    /// only this function can build. A plain check is enough here because
    /// `submit` is the only way in.
    fn validate(&self, o: &NewOrder) -> Result<(), Reject> {
        if o.qty == Qty::ZERO {
            return Err(Reject::ZeroQty);
        }
        if let Some(price) = o.limit_price() {
            if price.0 <= 0 {
                return Err(Reject::BadPrice(price));
            }
            if price.0 % self.tick != 0 {
                return Err(Reject::OffTick {
                    price,
                    tick: self.tick,
                });
            }
        }
        Ok(())
    }

    /// Would a `side` order at `price` trade with the best opposite order?
    fn would_cross(&self, side: Side, price: Price) -> bool {
        let best = match side {
            Side::Buy => self.best_ask(),
            Side::Sell => self.best_bid(),
        };
        best.is_some_and(|best| crosses(side, Some(price), best))
    }

    /// How much `o` could fill right now. Orders from the same trader don't
    /// count: matching would cancel them rather than trade with them.
    fn fillable(&self, o: &NewOrder) -> Qty {
        let limit = o.limit_price();
        self.levels(o.side.opposite()) // pub(super) in market_data.rs, so visible here
            .take_while(|&(&price, _)| crosses(o.side, limit, price))
            .flat_map(|(_, level)| &level.orders)
            .filter(|r| r.trader != o.trader)
            .map(|r| r.qty)
            .sum()
    }

    /// THE MATCHING LOOP. Walk the opposite side from its best price; at each
    /// level fill makers oldest first; stop when the taker is done or the
    /// next level is too expensive. Returns the taker's unfilled qty.
    fn match_order(&mut self, taker: OrderId, o: &NewOrder, sink: &mut impl EventSink) -> Qty {
        let mut left = o.qty;
        let limit = o.limit_price();

        // DISJOINT BORROWS (phase 4). We need to change one side of the book
        // AND the index at the same time. `self.side_mut(..)` would borrow
        // all of `self` and lock out `self.index`. Naming the fields lets the
        // compiler see the two borrows don't overlap.
        let (levels, index) = match o.side {
            Side::Buy => (&mut self.asks, &mut self.index),
            Side::Sell => (&mut self.bids, &mut self.index),
        };

        while left > Qty::ZERO {
            let Some(mut entry) = best_entry(levels, o.side.opposite()) else {
                break; // the other side is empty
            };
            let price = *entry.key();
            if !crosses(o.side, limit, price) {
                break; // the best remaining price is worse than the limit
            }
            let level = entry.get_mut();

            while left > Qty::ZERO {
                let Some(maker) = level.orders.front_mut() else {
                    break; // level used up
                };

                // SELF-TRADE PREVENTION. A trader's own orders mustn't trade
                // with each other (it fakes volume, and regulators care).
                // Policy here: cancel the resting order and keep going.
                if maker.trader == o.trader {
                    let gone = *maker; // a Copy, so the borrow of the queue ends here
                    level.orders.pop_front();
                    level.total -= gone.qty;
                    index.remove(&gone.id);
                    sink.emit(Event::Cancelled {
                        id: gone.id,
                        qty: gone.qty,
                        reason: CancelReason::SelfTrade,
                    });
                    continue;
                }

                // A FILL. The taker gets the maker's price.
                let qty = left.min(maker.qty);
                maker.qty -= qty;
                let (maker_id, maker_done) = (maker.id, maker.qty == Qty::ZERO);
                // `maker` (a borrow of level.orders) isn't used after this
                // line, so we're free to change the queue again.
                level.total -= qty;
                left -= qty;
                sink.emit(Event::Trade {
                    taker,
                    maker: maker_id,
                    side: o.side,
                    price,
                    qty,
                });
                if maker_done {
                    level.orders.pop_front();
                    index.remove(&maker_id);
                }
            }

            if level.orders.is_empty() {
                entry.remove(); // an empty level must not stay in the book
            }
        }
        left
    }

    /// Put the remainder of an order at the BACK of its level's queue.
    fn rest(
        &mut self,
        id: OrderId,
        trader: TraderId,
        side: Side,
        price: Price,
        qty: Qty,
        sink: &mut impl EventSink,
    ) {
        let level = self.side_mut(side).entry(price).or_default();
        level.orders.push_back(Resting { id, trader, qty });
        level.total += qty;
        // `level` borrowed all of self through side_mut(), but that borrow
        // ended at its last use above, so self.index is available again.
        self.index.insert(id, (side, price));
        sink.emit(Event::Rested { id, price, qty });
    }
}
