//! Matching behaviour, tested only through the public API.
//!   cargo test -p orderbook --test matching

mod common;

use common::*;
use orderbook::{Book, CancelReason, Event, OrderId, Price, Qty, Side, TimeInForce};

#[test]
fn price_time_priority() {
    let mut b = sample();
    let ev = submit(&mut b, lim(9, Side::Buy, 5, 1005));
    assert_eq!(trades(&ev), vec![(1, 1005, 3), (2, 1005, 2)]); // #1 before #2
    assert_eq!(b.order(OrderId(2)).map(|r| r.qty), Some(Qty(2))); // partly filled, still first
    assert_eq!(b.best_ask(), Some(Price(1005)));
    assert!(b.order(OrderId(1)).is_none());
}

#[test]
fn trades_at_maker_price_and_sweeps_levels() {
    let mut b = sample();
    let ev = submit(&mut b, lim(9, Side::Buy, 20, 1010));
    assert_eq!(trades(&ev), vec![(1, 1005, 3), (2, 1005, 4), (3, 1010, 5)]);
    assert!(ev.contains(&Event::Rested {
        id: OrderId(6),
        price: Price(1010),
        qty: Qty(8)
    }));
    assert_eq!(b.best_ask(), None); // the ask side is empty
    assert_eq!(b.best_bid(), Some(Price(1010)));
    assert_eq!(b.spread(), None);
}

#[test]
fn ioc_cancels_remainder() {
    let mut b = sample();
    let ev = submit(&mut b, lim_tif(9, Side::Sell, 10, 995, TimeInForce::Ioc));
    assert_eq!(trades(&ev), vec![(4, 1000, 6), (5, 995, 2)]);
    assert_eq!(cancels(&ev), vec![(6, 2, CancelReason::Unfilled)]);
    assert!(b.order(OrderId(6)).is_none());
    assert_eq!(b.best_bid(), None);
    assert_eq!(b.best_ask(), Some(Price(1005))); // other side untouched
}

#[test]
fn market_orders() {
    let mut b = sample();
    let ev = submit(&mut b, mkt(9, Side::Buy, 100));
    assert_eq!(trades(&ev).len(), 3);
    assert_eq!(cancels(&ev), vec![(6, 88, CancelReason::Unfilled)]);
    assert_eq!(b.best_ask(), None);

    let mut empty = Book::new(TICK);
    let ev = submit(&mut empty, mkt(9, Side::Sell, 5));
    assert_eq!(ev.len(), 2); // Accepted, then Cancelled
    assert_eq!(cancels(&ev), vec![(1, 5, CancelReason::Unfilled)]);
}

#[test]
fn fill_or_kill() {
    let mut b = sample();
    let before = depth(&b, Side::Sell);

    let ev = submit(&mut b, lim_tif(9, Side::Buy, 13, 1010, TimeInForce::Fok)); // 12 available
    assert_eq!(cancels(&ev), vec![(6, 13, CancelReason::FillOrKill)]);
    let ev = submit(&mut b, lim_tif(9, Side::Buy, 8, 1005, TimeInForce::Fok)); // 7 at <= 1005
    assert_eq!(cancels(&ev), vec![(7, 8, CancelReason::FillOrKill)]);
    assert_eq!(depth(&b, Side::Sell), before);

    let ev = submit(&mut b, lim_tif(9, Side::Buy, 12, 1010, TimeInForce::Fok));
    assert_eq!(trades(&ev).len(), 3);
    assert!(cancels(&ev).is_empty());
    assert_eq!(b.best_ask(), None);
}

#[test]
fn fok_ignores_own_liquidity() {
    let mut b = sample();
    // t1 owns #1 (3 @ 1005) and #3 (5 @ 1010); only #2's 4 are fillable for t1.
    let ev = submit(&mut b, lim_tif(1, Side::Buy, 5, 1010, TimeInForce::Fok));
    assert_eq!(cancels(&ev), vec![(6, 5, CancelReason::FillOrKill)]);
    assert_eq!(b.len(), 5);
}

#[test]
fn post_only() {
    let mut b = sample();
    let ev = submit(
        &mut b,
        lim_tif(9, Side::Buy, 1, 1005, TimeInForce::PostOnly),
    );
    assert_eq!(cancels(&ev), vec![(6, 1, CancelReason::WouldCross)]);
    assert!(trades(&ev).is_empty());

    let ev = submit(
        &mut b,
        lim_tif(9, Side::Buy, 1, 1000, TimeInForce::PostOnly),
    );
    assert!(ev.contains(&Event::Rested {
        id: OrderId(7),
        price: Price(1000),
        qty: Qty(1)
    }));
    assert_eq!(depth(&b, Side::Buy)[0], (1000, 7));
    // That #7 queues BEHIND #4 needs a look inside a level, which the public
    // API doesn't offer, so that part is a unit test in src/book/mod.rs.
}

#[test]
fn self_trade_prevention() {
    let mut b = sample();
    let ev = submit(&mut b, lim(1, Side::Buy, 5, 1005));
    assert_eq!(cancels(&ev), vec![(1, 3, CancelReason::SelfTrade)]);
    assert_eq!(trades(&ev), vec![(2, 1005, 4)]);
    assert!(ev.contains(&Event::Rested {
        id: OrderId(6),
        price: Price(1005),
        qty: Qty(1)
    }));
    // #3 (also t1) is at 1010, beyond the limit, so it's left alone.
    assert_eq!(b.best_ask(), Some(Price(1010)));
    assert!(b.order(OrderId(3)).is_some());
}
