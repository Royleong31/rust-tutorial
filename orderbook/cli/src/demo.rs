//! The capstone's scripted demo, unchanged.

use crate::session::{Printer, execute, print_chain};
use orderbook::{Book, Event, NewOrder, OrderId, Qty, Side, TraderId};

/// Lines starting with "//" are narration; the rest are commands.
pub(crate) const DEMO: &[&str] = &[
    "// Build a book. #2 and #3 are both at 1005; #2 arrived first, so it's first in the queue.",
    "t1 sell 5 @ 1010",
    "t2 sell 3 @ 1005",
    "t3 sell 4 @ 1005",
    "t4 buy 6 @ 1000",
    "t5 buy 2 @ 995",
    "book",
    "// A buy that crosses: it takes #2 then #3 (time priority) at 1005, and the 2 left over rest as the new best bid.",
    "t6 buy 9 @ 1005",
    "book",
    "// IOC sell: hits the bids from the best price down to its limit of 1000, then cancels the rest instead of resting.",
    "t7 sell 10 @ 1000 ioc",
    "// FOK: only 5 are offered at or below 1010, so the whole order is killed and the book doesn't change.",
    "t8 buy 100 @ 1010 fok",
    "// Post-only: at 1010 it would trade, so it's cancelled. At 1000 it rests.",
    "t8 buy 1 @ 1010 post",
    "t8 buy 1 @ 1000 post",
    "// Self-trade prevention: t1's buy would hit t1's own ask #1, so #1 is cancelled and the buy rests.",
    "t1 buy 2 @ 1010",
    "// Amend down keeps the queue position; amend up is refused; cancel removes; a second cancel finds nothing.",
    "amend 5 1",
    "amend 5 9",
    "cancel 5",
    "cancel 5",
    "// Bad requests are rejected without using up an order id.",
    "t9 buy 0 @ 1000",
    "t9 buy 1 @ 1003",
    "t9 hold 1 @ 1000",
    "t9 buy lots @ 1000",
    "// Some fresh asks for the last part.",
    "t2 sell 4 @ 1015",
    "t3 sell 6 @ 1020",
    "book",
];

pub fn run(book: &mut Book) {
    println!("== order book demo (tick size {}) ==", book.tick());
    for line in DEMO {
        if let Some(note) = line.strip_prefix("//") {
            println!("\n{}", note.trim());
            continue;
        }
        println!("  > {line}");
        if let Err(e) = execute(book, line, &mut Printer) {
            print_chain(&e);
        }
    }

    // The same engine with a different sink: collect the events, then work
    // out the average price the market order paid (its VWAP).
    println!("\nA market buy, with events collected in a Vec instead of printed:");
    let mut events = Vec::new();
    let order = NewOrder::market(TraderId(9), Side::Buy, Qty(7));
    let id = book
        .submit(order, &mut events)
        .expect("a market order with qty > 0 is valid");
    let (filled, notional) = events
        .iter()
        .filter_map(|e| match *e {
            Event::Trade { price, qty, .. } => Some((qty, price * qty)),
            _ => None,
        })
        .fold((Qty::ZERO, 0), |(q, n), (qty, value)| (q + qty, n + value));
    // Floats are fine for REPORTING a ratio, just not for keeping prices.
    let vwap = notional as f64 / filled.0 as f64;
    println!(
        "  {id}: {} events, filled {filled}, average price {vwap:.2}",
        events.len()
    );

    println!("\nMarket data:");
    println!(
        "  best bid {:?}, best ask {:?}, spread {:?}",
        book.best_bid(),
        book.best_ask(),
        book.spread()
    );
    println!("  bids {:?}", book.depth(Side::Buy, 5));
    println!("  asks {:?}", book.depth(Side::Sell, 5));
    println!(
        "  {} resting orders; #10 is {:?}",
        book.len(),
        book.order(OrderId(10))
    );
    print!("{book}");
    println!("\nRun with `-i` to type commands yourself, or pass a CSV file of orders.");
}
