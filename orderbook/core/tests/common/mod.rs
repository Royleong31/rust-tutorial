//! Helpers shared by the integration tests.
//!
//! Every file directly in tests/ is compiled as its own separate crate that
//! depends on `orderbook`, like any outside user would. Shared code goes in a
//! SUBDIRECTORY (tests/common/mod.rs) so cargo doesn't treat it as a test file
//! of its own; each test file pulls it in with `mod common;`.

// Not every test file uses every helper, which would otherwise warn.
#![allow(dead_code)]

use orderbook::{Book, CancelReason, Event, NewOrder, Price, Qty, Side, TimeInForce, TraderId};

pub const TICK: i64 = 5;

pub fn lim(trader: u32, side: Side, qty: u64, price: i64) -> NewOrder {
    lim_tif(trader, side, qty, price, TimeInForce::Gtc)
}

pub fn lim_tif(trader: u32, side: Side, qty: u64, price: i64, tif: TimeInForce) -> NewOrder {
    NewOrder::limit(TraderId(trader), side, Qty(qty), Price(price), tif)
}

pub fn mkt(trader: u32, side: Side, qty: u64) -> NewOrder {
    NewOrder::market(TraderId(trader), side, Qty(qty))
}

pub fn submit(book: &mut Book, o: NewOrder) -> Vec<Event> {
    let mut events = Vec::new();
    book.submit(o, &mut events).unwrap();
    events
}

/// (maker id, price, qty) of each trade, in order.
pub fn trades(events: &[Event]) -> Vec<(u64, i64, u64)> {
    events
        .iter()
        .filter_map(|e| match *e {
            Event::Trade {
                maker, price, qty, ..
            } => Some((maker.0, price.0, qty.0)),
            _ => None,
        })
        .collect()
}

/// (order id, open qty, reason) of each cancel, in order.
pub fn cancels(events: &[Event]) -> Vec<(u64, u64, CancelReason)> {
    events
        .iter()
        .filter_map(|e| match *e {
            Event::Cancelled { id, qty, reason } => Some((id.0, qty.0, reason)),
            _ => None,
        })
        .collect()
}

pub fn depth(book: &Book, side: Side) -> Vec<(i64, u64)> {
    book.depth(side, usize::MAX)
        .into_iter()
        .map(|(p, q)| (p.0, q.0))
        .collect()
}

/// ASK 1010: #3 t1 5
/// ASK 1005: #1 t1 3, #2 t2 4
/// BID 1000: #4 t3 6
/// BID  995: #5 t4 2
pub fn sample() -> Book {
    let mut b = Book::new(TICK);
    for o in [
        lim(1, Side::Sell, 3, 1005),
        lim(2, Side::Sell, 4, 1005),
        lim(1, Side::Sell, 5, 1010),
        lim(3, Side::Buy, 6, 1000),
        lim(4, Side::Buy, 2, 995),
    ] {
        submit(&mut b, o);
    }
    b
}
