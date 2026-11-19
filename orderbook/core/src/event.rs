//! What the book tells the world: events and rejects.
//!
//! Two different channels, on purpose:
//!
//!   Err(Reject)   "your REQUEST is invalid". Nothing happened: no id was
//!                 assigned and the book is untouched.
//!   Event         "here's what HAPPENED to your order". An IOC that finds
//!                 nothing to trade isn't an error; it was accepted and then
//!                 cancelled, and the client needs both facts.
//!
//! Real engines send events as "execution reports" to clients and as market
//! data to everyone. Here they go to whatever implements EventSink.

use crate::order::{NewOrder, Side};
use crate::types::{OrderId, Price, Qty};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    /// The client asked.
    Requested,
    /// The part of an IOC or market order that found nothing to trade.
    Unfilled,
    /// A FOK that couldn't fill completely.
    FillOrKill,
    /// A post-only order that would have traded.
    WouldCross,
    /// A resting order that its own trader's new order would have hit.
    SelfTrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Accepted {
        id: OrderId,
        order: NewOrder,
    },
    /// `side` is the TAKER's side, the "aggressor", which market data shows.
    Trade {
        taker: OrderId,
        maker: OrderId,
        side: Side,
        price: Price,
        qty: Qty,
    },
    Rested {
        id: OrderId,
        price: Price,
        qty: Qty,
    },
    Amended {
        id: OrderId,
        qty: Qty,
    },
    /// `qty` is what was still open when it was cancelled.
    Cancelled {
        id: OrderId,
        qty: Qty,
        reason: CancelReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    ZeroQty,
    BadPrice(Price),
    OffTick {
        price: Price,
        tick: i64,
    },
    UnknownOrder(OrderId),
    AmendMustReduce {
        id: OrderId,
        open: Qty,
        requested: Qty,
    },
}

impl fmt::Display for CancelReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            CancelReason::Requested => "requested",
            CancelReason::Unfilled => "nothing left to trade with",
            CancelReason::FillOrKill => "fill-or-kill could not fill completely",
            CancelReason::WouldCross => "post-only would have traded",
            CancelReason::SelfTrade => "self-trade prevention",
        })
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Event::Accepted { id, order } => write!(f, "{id} accepted: {order}"),
            Event::Trade {
                taker,
                maker,
                side,
                price,
                qty,
            } => {
                write!(
                    f,
                    "TRADE {qty} @ {price}  (taker {taker} {side}, maker {maker})"
                )
            }
            Event::Rested { id, price, qty } => write!(f, "{id} rests {qty} @ {price}"),
            Event::Amended { id, qty } => write!(f, "{id} amended, {qty} open"),
            Event::Cancelled { id, qty, reason } => {
                write!(f, "{id} cancelled with {qty} open: {reason}")
            }
        }
    }
}

impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Reject::ZeroQty => write!(f, "quantity must be greater than zero"),
            Reject::BadPrice(p) => write!(f, "price {p} must be positive"),
            Reject::OffTick { price, tick } => {
                write!(f, "price {price} is not a multiple of the tick size {tick}")
            }
            Reject::UnknownOrder(id) => write!(f, "no open order {id}"),
            Reject::AmendMustReduce {
                id,
                open,
                requested,
            } => {
                write!(
                    f,
                    "{id} has {open} open; an amend can only reduce it (asked for {requested})"
                )
            }
        }
    }
}

impl Error for Reject {}

/// Where events go. The book doesn't know or care whether that's a Vec, the
/// terminal, a network socket or a log file. The CLI crate adds a printer;
/// a future gateway would add one that writes to clients.
pub trait EventSink {
    fn emit(&mut self, e: Event);
}

/// Collect events, for tests and for code that inspects them afterwards.
impl EventSink for Vec<Event> {
    fn emit(&mut self, e: Event) {
        self.push(e);
    }
}
