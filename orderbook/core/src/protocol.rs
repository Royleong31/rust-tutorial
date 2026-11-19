//! The text protocol: parse at the boundary.
//!
//! Text comes in, gets turned into typed values ONCE, and everything past this
//! point works with types that can't be malformed.
//!
//!   t1 buy 10 @ 1000           limit order, GTC
//!   t1 buy 10 @ 1000 ioc       ...or ioc / fok / post
//!   t2 sell 5 mkt              market order
//!   cancel 3                   amend 3 2                book            help
//!
//! This module only PARSES. Running a command (and printing `book` or
//! `help`) is the CLI's job, since that's I/O.

use crate::order::{NewOrder, OrderType, Side, TimeInForce};
use crate::types::{OrderId, Price, Qty, TraderId};
use std::error::Error;
use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

pub const HELP: &str = "\
  commands:
    t1 buy 10 @ 1000 [gtc|ioc|fok|post]   limit order from trader 1
    t2 sell 5 mkt                         market order from trader 2
    cancel 3     amend 3 2     book     help     (Ctrl-D to quit)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Submit(NewOrder),
    Cancel(OrderId),
    Amend(OrderId, Qty),
    Show,
    Help,
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    Empty,
    /// `got` is a String, not a &str borrowed from the line: an error that
    /// borrows its input can't outlive the input, and errors tend to travel.
    Unknown {
        what: &'static str,
        got: String,
    },
    BadNumber {
        what: &'static str,
        source: ParseIntError,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty line"),
            ParseError::Unknown { what, got } => write!(f, "unknown {what} {got:?} (try `help`)"),
            ParseError::BadNumber { what, .. } => write!(f, "{what} is not a valid number"),
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ParseError::BadNumber { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Parse any integer type (a generic with a bound on an associated type),
/// naming the field if it fails.
fn num<T: FromStr<Err = ParseIntError>>(what: &'static str, s: &str) -> Result<T, ParseError> {
    s.parse()
        .map_err(|source| ParseError::BadNumber { what, source })
}

/// Turn one line of text into a Command.
pub fn parse_command(line: &str) -> Result<Command, ParseError> {
    let line = line.trim().to_ascii_lowercase();
    let words: Vec<&str> = line.split_whitespace().collect();

    // SLICE PATTERNS: match on the shape of the word list. `rest @ ..` binds
    // "everything after", which is itself a slice to match on.
    match words.as_slice() {
        [] => Err(ParseError::Empty),
        ["book"] => Ok(Command::Show),
        ["help"] => Ok(Command::Help),
        ["cancel", id] => Ok(Command::Cancel(OrderId(num(
            "order id",
            id.trim_start_matches('#'),
        )?))),
        ["amend", id, qty] => Ok(Command::Amend(
            OrderId(num("order id", id.trim_start_matches('#'))?),
            Qty(num("qty", qty)?),
        )),
        [trader, side, qty, rest @ ..] if trader.starts_with('t') => {
            let trader = TraderId(num("trader", &trader[1..])?);
            let side: Side = side.parse()?; // FromStr, defined next to Side in order.rs
            let qty = Qty(num("qty", qty)?);
            let kind = match rest {
                ["mkt"] => OrderType::Market,
                ["@", price] => OrderType::Limit {
                    price: Price(num("price", price)?),
                    tif: TimeInForce::Gtc,
                },
                ["@", price, tif] => {
                    let tif: TimeInForce = tif.parse()?;
                    OrderType::Limit {
                        price: Price(num("price", price)?),
                        tif,
                    }
                }
                _ => {
                    return Err(ParseError::Unknown {
                        what: "order type",
                        got: rest.join(" "),
                    });
                }
            };
            Ok(Command::Submit(NewOrder {
                trader,
                side,
                qty,
                kind,
            }))
        }
        _ => Err(ParseError::Unknown {
            what: "command",
            got: line.clone(),
        }),
    }
}
