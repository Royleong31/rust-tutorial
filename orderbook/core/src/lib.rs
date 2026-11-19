//! A limit order book and matching engine for one instrument.
//!
//! This is the capstone (`src/bin/capstone_order_book.rs`) split into a
//! LIBRARY crate. The library is pure logic: no printing, no files, no
//! threads, no clock. You hand the [`Book`] an order and an [`EventSink`]; it
//! matches, and reports what happened through the sink. Everything with side
//! effects (reading a CSV file, talking to the terminal) lives in the
//! `orderbook-cli` crate next door. That split is what lets you later put the
//! same book behind a TCP gateway, or inside an engine thread, without
//! changing this crate.
//!
//! Each module is one file:
//!
//! ```text
//! orderbook                src/lib.rs                this file: docs, modules, re-exports
//! ├── types                src/types.rs              Price, Qty, OrderId, TraderId
//! ├── order                src/order.rs              Side, TimeInForce, OrderType, NewOrder, Resting
//! ├── event                src/event.rs              Event, CancelReason, Reject, EventSink
//! ├── book                 src/book/mod.rs           Book, Level, shared helpers, Display
//! │   ├── submit           src/book/submit.rs        validate, match, rest
//! │   ├── cancel           src/book/cancel.rs        cancel, amend
//! │   ├── market_data      src/book/market_data.rs   best bid/ask, depth, lookups
//! │   └── invariants       src/book/invariants.rs    check()
//! └── protocol             src/protocol.rs           the text command language
//! ```
//!
//! # Example
//!
//! This example is a DOC TEST: `cargo test -p orderbook` compiles and runs it,
//! so the documentation can't silently go out of date.
//!
//! ```
//! use orderbook::{Book, Event, NewOrder, Price, Qty, Side, TimeInForce, TraderId};
//!
//! let mut book = Book::new(5); // tick size 5
//! let mut events: Vec<Event> = Vec::new(); // a Vec is an EventSink
//!
//! book.submit(NewOrder::limit(TraderId(1), Side::Sell, Qty(5), Price(1010), TimeInForce::Gtc), &mut events).unwrap();
//! book.submit(NewOrder::limit(TraderId(2), Side::Buy, Qty(2), Price(1010), TimeInForce::Gtc), &mut events).unwrap();
//!
//! assert!(events.iter().any(|e| matches!(e, Event::Trade { qty: Qty(2), price: Price(1010), .. })));
//! assert_eq!(book.depth(Side::Sell, 1), vec![(Price(1010), Qty(3))]);
//! ```

// The modules are PRIVATE (`mod`, not `pub mod`). Users never write
// `orderbook::book::Book`; they use the re-exports below. So you can move
// code between files, or split a file in two, without breaking anyone.
mod book;
mod event;
mod order;
mod protocol;
mod types;

// RE-EXPORTS: the crate's public API, all in one place.
pub use book::{Book, Depth};
pub use event::{CancelReason, Event, EventSink, Reject};
pub use order::{NewOrder, OrderType, Resting, Side, TimeInForce};
pub use protocol::{Command, HELP, ParseError, parse_command};
pub use types::{OrderId, Price, Qty, TraderId};
