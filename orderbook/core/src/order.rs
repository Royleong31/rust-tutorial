//! Orders: side, order type, time in force.
//!
//! Each enum lists every possibility, and every `match` on it must handle all
//! of them. Add a variant and the compiler shows you every place that needs
//! a decision.

use crate::protocol::ParseError; // `crate::` = the root of THIS crate (lib.rs)
use crate::types::{OrderId, Price, Qty, TraderId};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn opposite(self) -> Side {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }
}

/// What happens to the part of an order that can't trade on arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// Good 'til cancelled: trade what you can, rest the remainder.
    Gtc,
    /// Immediate or cancel: trade what you can, cancel the remainder.
    Ioc,
    /// Fill or kill: trade ALL of it now, or none of it.
    Fok,
    /// Post-only: must NOT trade on arrival (maker only). If it would, cancel
    /// it. Strictly a flag rather than a time in force, but many venues put
    /// it in the same field.
    PostOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    /// Trade at `price` or better.
    Limit { price: Price, tif: TimeInForce },
    /// Trade at any price, never rest. Like IOC with no price limit.
    Market,
}

/// What a client sends. There's no id yet: the book assigns one when it
/// accepts the order. Every field is Copy, so NewOrder can be Copy too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewOrder {
    pub trader: TraderId,
    pub side: Side,
    pub qty: Qty,
    pub kind: OrderType,
}

impl NewOrder {
    /// A limit order.
    pub fn limit(
        trader: TraderId,
        side: Side,
        qty: Qty,
        price: Price,
        tif: TimeInForce,
    ) -> NewOrder {
        NewOrder {
            trader,
            side,
            qty,
            kind: OrderType::Limit { price, tif },
        }
    }

    /// A market order.
    pub fn market(trader: TraderId, side: Side, qty: Qty) -> NewOrder {
        NewOrder {
            trader,
            side,
            qty,
            kind: OrderType::Market,
        }
    }

    /// The worst price this order accepts, or None for "any price".
    pub fn limit_price(&self) -> Option<Price> {
        match self.kind {
            OrderType::Limit { price, .. } => Some(price),
            OrderType::Market => None,
        }
    }
}

/// An order waiting in the book. Its price is the key of its level and its
/// time priority is its position in the level's queue, so neither is stored.
/// `qty` is what's LEFT, and shrinks as the order fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resting {
    pub id: OrderId,
    pub trader: TraderId,
    pub qty: Qty,
}

// FromStr is the standard "parse from text" trait: implementing it makes
// `"buy".parse::<Side>()` work. Both the text protocol and the CLI's CSV
// reader use these, so the accepted spellings live in ONE place.
impl FromStr for Side {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Side, ParseError> {
        match s.to_ascii_lowercase().as_str() {
            "buy" => Ok(Side::Buy),
            "sell" => Ok(Side::Sell),
            _ => Err(ParseError::Unknown {
                what: "side",
                got: s.to_string(),
            }),
        }
    }
}

impl FromStr for TimeInForce {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<TimeInForce, ParseError> {
        match s.to_ascii_lowercase().as_str() {
            "gtc" => Ok(TimeInForce::Gtc),
            "ioc" => Ok(TimeInForce::Ioc),
            "fok" => Ok(TimeInForce::Fok),
            "post" => Ok(TimeInForce::PostOnly),
            _ => Err(ParseError::Unknown {
                what: "time in force",
                got: s.to_string(),
            }),
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        })
    }
}

impl fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            TimeInForce::Gtc => "GTC",
            TimeInForce::Ioc => "IOC",
            TimeInForce::Fok => "FOK",
            TimeInForce::PostOnly => "POST",
        })
    }
}

impl fmt::Display for NewOrder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.trader, self.side, self.qty)?;
        match self.kind {
            OrderType::Limit { price, tif } => write!(f, " @ {price} {tif}"),
            OrderType::Market => write!(f, " MKT"),
        }
    }
}
