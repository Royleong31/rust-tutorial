//! Cancel, amend, rejects and market data.
//!   cargo test -p orderbook --test orders

mod common;

use common::*;
use orderbook::{Book, CancelReason, OrderId, Price, Qty, Reject, Resting, Side, TraderId};

#[test]
fn cancel() {
    let mut b = sample();
    let mut ev = Vec::new();
    let gone = b.cancel(OrderId(2), &mut ev).unwrap();
    assert_eq!(
        gone,
        Resting {
            id: OrderId(2),
            trader: TraderId(2),
            qty: Qty(4)
        }
    );
    assert_eq!(depth(&b, Side::Sell), vec![(1005, 3), (1010, 5)]);

    b.cancel(OrderId(1), &mut ev).unwrap();
    assert_eq!(b.best_ask(), Some(Price(1010))); // empty level removed
    assert_eq!(
        b.cancel(OrderId(1), &mut ev),
        Err(Reject::UnknownOrder(OrderId(1)))
    );
    assert_eq!(
        cancels(&ev),
        vec![
            (2, 4, CancelReason::Requested),
            (1, 3, CancelReason::Requested)
        ]
    );
}

#[test]
fn amend_keeps_priority() {
    let mut b = sample();
    let mut ev = Vec::new();
    b.amend(OrderId(1), Qty(1), &mut ev).unwrap();
    assert_eq!(depth(&b, Side::Sell)[0], (1005, 5));
    let ev = submit(&mut b, lim(9, Side::Buy, 2, 1005));
    assert_eq!(trades(&ev), vec![(1, 1005, 1), (2, 1005, 1)]); // #1 still first

    let mut ev = Vec::new();
    assert_eq!(
        b.amend(OrderId(2), Qty(9), &mut ev),
        Err(Reject::AmendMustReduce {
            id: OrderId(2),
            open: Qty(3),
            requested: Qty(9)
        })
    );
    assert_eq!(b.amend(OrderId(2), Qty(0), &mut ev), Err(Reject::ZeroQty));
    assert_eq!(
        b.amend(OrderId(99), Qty(1), &mut ev),
        Err(Reject::UnknownOrder(OrderId(99)))
    );
    assert!(ev.is_empty());
}

#[test]
fn rejects_change_nothing() {
    let mut b = sample();
    let mut ev = Vec::new();
    assert_eq!(
        b.submit(lim(9, Side::Buy, 0, 1000), &mut ev),
        Err(Reject::ZeroQty)
    );
    assert_eq!(
        b.submit(mkt(9, Side::Buy, 0), &mut ev),
        Err(Reject::ZeroQty)
    );
    assert_eq!(
        b.submit(lim(9, Side::Buy, 1, 1003), &mut ev),
        Err(Reject::OffTick {
            price: Price(1003),
            tick: TICK
        })
    );
    assert_eq!(
        b.submit(lim(9, Side::Buy, 1, 0), &mut ev),
        Err(Reject::BadPrice(Price(0)))
    );
    assert!(ev.is_empty());
    assert_eq!(b.submit(lim(9, Side::Buy, 1, 990), &mut ev), Ok(OrderId(6))); // no ids used up
}

#[test]
fn market_data() {
    let b = sample();
    assert_eq!(depth(&b, Side::Buy), vec![(1000, 6), (995, 2)]);
    assert_eq!(b.depth(Side::Sell, 1), vec![(Price(1005), Qty(7))]);
    assert_eq!(b.spread(), Some(5));
    assert_eq!(b.len(), 5);
    assert!(Book::new(TICK).depth(Side::Buy, 3).is_empty());
    assert!(Book::new(TICK).is_empty());
}
