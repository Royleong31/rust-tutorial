//! CAPSTONE — A limit order book and matching engine (phases 1–9 together)
//!
//!   cargo run  --bin capstone_order_book          scripted demo
//!   cargo run  --bin capstone_order_book -- -i    type commands yourself
//!   cargo test --bin capstone_order_book
//!
//! Phase 6 built the skeleton of a book (add, cancel, best bid/ask) and its
//! exercises added depth and a market buy. This file is the whole thing: one
//! instrument, one thread, everything in memory, but with the behaviour a
//! real matching engine has. Read it top to bottom; each section names the
//! phases it leans on.
//!
//!   ORDER BOOK CONCEPT                            RUST CONCEPT                          PHASE
//!   prices are integers, with a tick size         integers, not floats                  1
//!   Price / Qty / OrderId can't be mixed up       newtypes + operator traits            2, 8
//!   limit vs market, GTC / IOC / FOK / post-only  enums + exhaustive match              2
//!   orders move into the book, cancel hands back  ownership, Copy                       3
//!   matching mutates one side AND the index       disjoint &mut borrows                 4
//!   market data is a view into the book           returning references, '_              5
//!   price-time priority, fast best price/cancel   BTreeMap + VecDeque + HashMap         6
//!   rejects (bad request) vs cancels (outcome)    Result vs events, layered errors, ?   7
//!   the book must never be inconsistent           invariants + debug_assert!            7
//!   execution reports go "somewhere"              a trait + static dispatch             8
//!   two iterator types behind one return type     Box<dyn Iterator>                     9

use std::collections::btree_map::OccupiedEntry;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::iter::Sum;
use std::num::ParseIntError;
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};
use std::str::FromStr;

/// The smallest allowed price step. Prices must be multiples of it.
const TICK: i64 = 5;

fn main() {
    let mut book = Book::new(TICK);
    if std::env::args().any(|a| a == "-i") {
        repl(&mut book);
    } else {
        demo(&mut book);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 0. What an order book is
// ─────────────────────────────────────────────────────────────────────────────
//
// A list of everyone who wants to BUY (bids) and SELL (asks) one instrument,
// each at a price they chose. Orders at the same price form a LEVEL.
//
//          price   qty   queue (oldest first)
//   ASK     1010     5   #1:5
//   ASK     1005     7   #2:3 #3:4         <- best ask: the cheapest seller
//   ───────────── spread 5 ─────────────
//   BID     1000     6   #4:6              <- best bid: the highest buyer
//   BID      995     2   #5:2
//
// - An incoming order that trades immediately is a TAKER (it removes
//   liquidity). An order that waits in the book is a MAKER (it provides it).
// - A buy at 1005 or higher CROSSES the spread and trades with the asks. The
//   trade happens at the MAKER's price, so a buy limited at 1010 pays 1005.
// - PRICE-TIME PRIORITY: the best price trades first; at the same price, the
//   order that arrived first trades first (FIFO). That's the fairness rule of
//   most exchanges, and it's why each level is a queue.
// - After matching, the book is never crossed: best bid < best ask.

// ─────────────────────────────────────────────────────────────────────────────
// 1. Units: newtypes, so a price can't be passed where a qty is expected
// ─────────────────────────────────────────────────────────────────────────────
//
// Phases 1, 2, 8. Prices are integers in the instrument's smallest unit (say
// cents). Never f64: 0.1 + 0.2 != 0.3, and f64 isn't Ord, so it couldn't
// even be a BTreeMap key.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Price(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
struct Qty(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct OrderId(u64);

/// Who sent the order. Needed for self-trade prevention (section 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TraderId(u32);

impl Qty {
    const ZERO: Qty = Qty(0);
}

// Only the arithmetic that MEANS something is implemented:
//   Qty ± Qty -> Qty        Price - Price -> i64 (a distance)        Price × Qty -> i64 (notional)
// Price + Price would compile to nonsense, so it doesn't compile at all.
//
// Qty subtraction underflowing (taking more than there is) panics in debug
// builds. That's what we want: it can only happen through a bug in this file.
impl Add for Qty {
    type Output = Qty;
    fn add(self, rhs: Qty) -> Qty {
        Qty(self.0 + rhs.0)
    }
}

impl Sub for Qty {
    type Output = Qty;
    fn sub(self, rhs: Qty) -> Qty {
        Qty(self.0 - rhs.0)
    }
}

impl AddAssign for Qty {
    fn add_assign(&mut self, rhs: Qty) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Qty {
    fn sub_assign(&mut self, rhs: Qty) {
        self.0 -= rhs.0;
    }
}

impl Sum for Qty {
    fn sum<I: Iterator<Item = Qty>>(iter: I) -> Qty {
        iter.fold(Qty::ZERO, |a, b| a + b)
    }
}

impl Sub for Price {
    type Output = i64;
    fn sub(self, rhs: Price) -> i64 {
        self.0 - rhs.0
    }
}

impl Mul<Qty> for Price {
    type Output = i64;
    fn mul(self, rhs: Qty) -> i64 {
        self.0 * rhs.0 as i64
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // `pad` (instead of write!) makes width flags like {:>6} work.
        f.pad(&self.0.to_string())
    }
}

impl fmt::Display for Qty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&self.0.to_string())
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl fmt::Display for TraderId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Orders: side, order type, time in force
// ─────────────────────────────────────────────────────────────────────────────
//
// Phases 2, 3. Each enum lists every possibility, and every `match` on it
// must handle all of them. Add a variant and the compiler shows you every
// place that needs a decision.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Buy,
    Sell,
}

impl Side {
    fn opposite(self) -> Side {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }
}

/// What happens to the part of an order that can't trade on arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeInForce {
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
enum OrderType {
    /// Trade at `price` or better.
    Limit { price: Price, tif: TimeInForce },
    /// Trade at any price, never rest. Like IOC with no price limit.
    Market,
}

/// What a client sends. There's no id yet: the engine assigns one when it
/// accepts the order. Every field is Copy, so NewOrder can be Copy too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NewOrder {
    trader: TraderId,
    side: Side,
    qty: Qty,
    kind: OrderType,
}

impl NewOrder {
    /// The worst price this order accepts, or None for "any price".
    fn limit_price(&self) -> Option<Price> {
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
struct Resting {
    id: OrderId,
    trader: TraderId,
    qty: Qty,
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

// ─────────────────────────────────────────────────────────────────────────────
// 3. What the engine tells the world: events and rejects
// ─────────────────────────────────────────────────────────────────────────────
//
// Phases 7, 8. Two different channels, on purpose:
//
//   Err(Reject)   "your REQUEST is invalid". Nothing happened: no id was
//                 assigned and the book is untouched.
//   Event         "here's what HAPPENED to your order". An IOC that finds
//                 nothing to trade isn't an error; it was accepted and then
//                 cancelled, and the client needs both facts.
//
// Real engines send events as "execution reports" to clients and as market
// data to everyone. Here they go to whatever implements EventSink.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CancelReason {
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
enum Event {
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
enum Reject {
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
/// terminal, a network socket or a log file.
trait EventSink {
    fn emit(&mut self, e: Event);
}

/// Collect events, for tests and for code that inspects them afterwards.
impl EventSink for Vec<Event> {
    fn emit(&mut self, e: Event) {
        self.push(e);
    }
}

/// Print events as they happen.
struct Printer;

impl EventSink for Printer {
    fn emit(&mut self, e: Event) {
        println!("    {e}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. The book
// ─────────────────────────────────────────────────────────────────────────────
//
// Phase 6's layout, plus a cached total per level:
//
//   bids:  BTreeMap<Price, Level>     sorted; best bid = LAST key
//   asks:  BTreeMap<Price, Level>     sorted; best ask = FIRST key
//   Level: VecDeque<Resting> + total  FIFO queue = time priority
//   index: HashMap<OrderId, (Side, Price)>
//                                     cancel/amend jump straight to the level
//
// The book OWNS every resting order (phase 3). Nothing else holds a pointer
// to one, so there's no Rc or RefCell anywhere: outside code refers to an
// order by its OrderId and asks the book. (Phase 9's slab takes that idea
// further; see the extensions at the bottom.)

/// One price level.
#[derive(Debug, Default)]
struct Level {
    orders: VecDeque<Resting>,
    /// Sum of `orders[..].qty`, kept up to date so depth needs no summing.
    /// A cache like this is a promise every method must keep (section 8).
    total: Qty,
}

#[derive(Debug)]
struct Book {
    tick: i64,
    bids: BTreeMap<Price, Level>,
    asks: BTreeMap<Price, Level>,
    index: HashMap<OrderId, (Side, Price)>,
    next_id: u64,
}

impl Book {
    fn new(tick: i64) -> Book {
        assert!(tick > 0, "tick size must be positive"); // a programmer error, so panic
        Book {
            tick,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            index: HashMap::new(),
            next_id: 1,
        }
    }

    fn side(&self, side: Side) -> &BTreeMap<Price, Level> {
        match side {
            Side::Buy => &self.bids,
            Side::Sell => &self.asks,
        }
    }

    fn side_mut(&mut self, side: Side) -> &mut BTreeMap<Price, Level> {
        match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        }
    }
}

/// The best level of `levels`, which holds `side`'s orders: the HIGHEST bid
/// or the LOWEST ask. An OccupiedEntry lets the caller change the level and
/// then delete it without looking it up again. Its lifetime ('_) ties it to
/// the &mut borrow of `levels` (phase 5).
fn best_entry(
    levels: &mut BTreeMap<Price, Level>,
    side: Side,
) -> Option<OccupiedEntry<'_, Price, Level>> {
    match side {
        Side::Buy => levels.last_entry(),
        Side::Sell => levels.first_entry(),
    }
}

/// Can a taker on `side`, with this limit, trade at a level priced `level`?
fn crosses(side: Side, limit: Option<Price>, level: Price) -> bool {
    match (side, limit) {
        (_, None) => true, // market order: any price
        (Side::Buy, Some(limit)) => level <= limit,
        (Side::Sell, Some(limit)) => level >= limit,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Submitting: validate, match, then rest or cancel what's left
// ─────────────────────────────────────────────────────────────────────────────
//
// Phases 3, 4, 7. The order of the steps matters: everything that can fail
// is checked BEFORE anything changes. A function that errors halfway through
// a mutation leaves half-updated state behind, and in a book that means
// orders that exist in the index but not in a level, or totals that lie.

impl Book {
    fn submit(&mut self, o: NewOrder, sink: &mut impl EventSink) -> Result<OrderId, Reject> {
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
        self.levels(o.side.opposite())
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

// ─────────────────────────────────────────────────────────────────────────────
// 6. Cancel and amend
// ─────────────────────────────────────────────────────────────────────────────
//
// Phases 3, 6, 7. Cancel REMOVES the order from the book and gives it back to
// the caller by value: ownership leaves the book. Amend changes it in place.
//
// Amend can only REDUCE quantity. A smaller order keeps its place in the
// queue, since that only hurts the trader who amended. Increasing quantity or
// changing price would let an order jump ahead of everyone who queued after
// it, so venues treat that as cancel + new order, which goes to the BACK.

impl Book {
    fn cancel(&mut self, id: OrderId, sink: &mut impl EventSink) -> Result<Resting, Reject> {
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
    fn amend(&mut self, id: OrderId, qty: Qty, sink: &mut impl EventSink) -> Result<(), Reject> {
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

// ─────────────────────────────────────────────────────────────────────────────
// 7. Market data: queries that borrow the book
// ─────────────────────────────────────────────────────────────────────────────
//
// Phases 5, 6, 8, 9. All of these take &self, so any number can run at once
// and none can change the book.

/// (price, total qty) per level, best first.
type Depth = Vec<(Price, Qty)>;

impl Book {
    fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next_back().copied()
    }

    fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
    }

    /// `?` works on Option too: if either side is empty, return None.
    fn spread(&self) -> Option<i64> {
        Some(self.best_ask()? - self.best_bid()?)
    }

    /// One side's levels, best price first.
    ///
    /// The two arms produce DIFFERENT iterator types (Rev<Iter<..>> and
    /// Iter<..>), and a function returns ONE type, so `impl Iterator` can't
    /// cover both. Boxing them behind `dyn Iterator` (phases 8, 9) can, at
    /// the cost of a heap allocation and dynamic calls. The `'_` says the
    /// iterator borrows from `self`, so the book can't change while it lives.
    fn levels(&self, side: Side) -> Box<dyn Iterator<Item = (&Price, &Level)> + '_> {
        match side {
            Side::Buy => Box::new(self.bids.iter().rev()),
            Side::Sell => Box::new(self.asks.iter()),
        }
    }

    /// The top `n` levels of one side: what an exchange publishes as
    /// "market by price". A Vec, because market data is a snapshot that
    /// outlives the borrow of the book.
    fn depth(&self, side: Side, n: usize) -> Depth {
        self.levels(side)
            .take(n)
            .map(|(&price, level)| (price, level.total))
            .collect()
    }

    /// A borrowed view of one resting order. The returned reference can't
    /// outlive the book (lifetime elision ties it to &self, phase 5).
    fn order(&self, id: OrderId) -> Option<&Resting> {
        let &(side, price) = self.index.get(&id)?;
        self.side(side)
            .get(&price)?
            .orders
            .iter()
            .find(|r| r.id == id)
    }

    /// Number of resting orders.
    fn len(&self) -> usize {
        self.index.len()
    }
}

impl fmt::Display for Book {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn queue(level: &Level) -> String {
            level
                .orders
                .iter()
                .map(|r| format!("{}:{}", r.id, r.qty))
                .collect::<Vec<_>>()
                .join(" ")
        }
        // Asks high to low, so the best ask sits just above the spread line.
        for (price, level) in self.asks.iter().rev() {
            writeln!(
                f,
                "    ASK {price:>6} {:>5}   {}",
                level.total,
                queue(level)
            )?;
        }
        match self.spread() {
            Some(s) => writeln!(f, "    ────────── spread {s} ──────────")?,
            None => writeln!(f, "    ────────── no spread ──────────")?,
        }
        for (price, level) in self.bids.iter().rev() {
            writeln!(
                f,
                "    BID {price:>6} {:>5}   {}",
                level.total,
                queue(level)
            )?;
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Invariants: what must ALWAYS be true
// ─────────────────────────────────────────────────────────────────────────────
//
// Phase 7. The book keeps the same facts in several places (a level's queue
// and its total; the levels and the index). Each method has to update all of
// them, and forgetting one is the classic order book bug. So every mutating
// method ends with debug_assert_eq!(self.check(), Ok(())):
//   - on in `cargo run` and `cargo test`, so a bug is caught at the first
//     operation that causes it, not three hundred operations later
//   - compiled out of `--release` builds, because it scans the whole book
// The randomized test at the bottom runs thousands of operations through it.

impl Book {
    fn check(&self) -> Result<(), String> {
        for (side, levels) in [(Side::Buy, &self.bids), (Side::Sell, &self.asks)] {
            for (&price, level) in levels {
                if level.orders.is_empty() {
                    return Err(format!("empty {side} level at {price}"));
                }
                let sum: Qty = level.orders.iter().map(|r| r.qty).sum();
                if sum != level.total {
                    return Err(format!(
                        "{side} {price}: total says {} but orders add up to {sum}",
                        level.total
                    ));
                }
                for r in &level.orders {
                    if r.qty == Qty::ZERO {
                        return Err(format!("{} rests with zero qty", r.id));
                    }
                    if self.index.get(&r.id) != Some(&(side, price)) {
                        return Err(format!(
                            "{} is at {side} {price} but the index disagrees",
                            r.id
                        ));
                    }
                }
            }
        }
        let resting: usize = self
            .bids
            .values()
            .chain(self.asks.values())
            .map(|l| l.orders.len())
            .sum();
        if resting != self.index.len() {
            return Err(format!(
                "{resting} orders rest but the index has {}",
                self.index.len()
            ));
        }
        if let (Some(bid), Some(ask)) = (self.best_bid(), self.best_ask()) {
            if bid >= ask {
                return Err(format!("book is crossed: bid {bid} >= ask {ask}"));
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. The text protocol: parse at the boundary, layered errors
// ─────────────────────────────────────────────────────────────────────────────
//
// Phase 7. Text comes in, gets turned into typed values ONCE, and everything
// past this point works with types that can't be malformed.
//
//   t1 buy 10 @ 1000           limit order, GTC
//   t1 buy 10 @ 1000 ioc       ...or ioc / fok / post
//   t2 sell 5 mkt              market order
//   cancel 3                   amend 3 2                book            help
//
// Two layers of error, joined by From so `?` converts between them:
//
//   EngineError ── Parse(ParseError) ── BadNumber { source: ParseIntError }
//               └─ Reject(Reject)

const HELP: &str = "\
  commands:
    t1 buy 10 @ 1000 [gtc|ioc|fok|post]   limit order from trader 1
    t2 sell 5 mkt                         market order from trader 2
    cancel 3     amend 3 2     book     help     (Ctrl-D to quit)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Submit(NewOrder),
    Cancel(OrderId),
    Amend(OrderId, Qty),
    Show,
    Help,
}

#[derive(Debug, PartialEq)]
enum ParseError {
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

#[derive(Debug, PartialEq)]
enum EngineError {
    Parse(ParseError),
    Reject(Reject),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EngineError::Parse(_) => write!(f, "could not parse command"),
            EngineError::Reject(_) => write!(f, "rejected"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EngineError::Parse(e) => Some(e),
            EngineError::Reject(e) => Some(e),
        }
    }
}

impl From<ParseError> for EngineError {
    fn from(e: ParseError) -> Self {
        EngineError::Parse(e)
    }
}

impl From<Reject> for EngineError {
    fn from(e: Reject) -> Self {
        EngineError::Reject(e)
    }
}

/// Parse any integer type (phase 8: a generic with a bound on an
/// associated type), naming the field if it fails.
fn num<T: FromStr<Err = ParseIntError>>(what: &'static str, s: &str) -> Result<T, ParseError> {
    s.parse()
        .map_err(|source| ParseError::BadNumber { what, source })
}

fn parse_command(line: &str) -> Result<Command, ParseError> {
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
            let side = match *side {
                "buy" => Side::Buy,
                "sell" => Side::Sell,
                other => {
                    return Err(ParseError::Unknown {
                        what: "side",
                        got: other.to_string(),
                    });
                }
            };
            let qty = Qty(num("qty", qty)?);
            let kind = match rest {
                ["mkt"] => OrderType::Market,
                ["@", price] => OrderType::Limit {
                    price: Price(num("price", price)?),
                    tif: TimeInForce::Gtc,
                },
                ["@", price, tif] => {
                    let tif = match *tif {
                        "gtc" => TimeInForce::Gtc,
                        "ioc" => TimeInForce::Ioc,
                        "fok" => TimeInForce::Fok,
                        "post" => TimeInForce::PostOnly,
                        other => {
                            return Err(ParseError::Unknown {
                                what: "time in force",
                                got: other.to_string(),
                            });
                        }
                    };
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

/// Parse one line and run it. The two steps fail with two different error
/// types, and `?` turns both into EngineError through the From impls.
fn execute(book: &mut Book, line: &str, sink: &mut impl EventSink) -> Result<(), EngineError> {
    match parse_command(line)? {
        Command::Submit(o) => {
            book.submit(o, sink)?;
        }
        Command::Cancel(id) => {
            book.cancel(id, sink)?;
        }
        Command::Amend(id, qty) => book.amend(id, qty, sink)?,
        Command::Show => print!("{book}"),
        Command::Help => println!("{HELP}"),
    }
    Ok(())
}

/// Print an error and everything that caused it (phase 7).
fn print_chain(e: &dyn Error) {
    print!("    error: {e}");
    let mut cause = e.source();
    while let Some(c) = cause {
        print!(": {c}");
        cause = c.source();
    }
    println!();
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Running it: a scripted demo and an interactive mode
// ─────────────────────────────────────────────────────────────────────────────

/// Lines starting with "//" are narration; the rest are commands.
const DEMO: &[&str] = &[
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

fn demo(book: &mut Book) {
    println!("== order book demo (tick size {}) ==", book.tick);
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
    let order = NewOrder {
        trader: TraderId(9),
        side: Side::Buy,
        qty: Qty(7),
        kind: OrderType::Market,
    };
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
    println!("\nRun with `-- -i` to type commands yourself.");
}

fn repl(book: &mut Book) {
    println!("== order book (tick size {}) ==\n{HELP}", book.tick);
    // One String reused for every line, instead of allocating a new one each time.
    let mut line = String::new();
    loop {
        print!("> ");
        io::stdout().flush().ok(); // print! doesn't flush on its own
        line.clear();
        match io::stdin().read_line(&mut line) {
            Ok(0) | Err(_) => break, // Ctrl-D, or stdin went away
            Ok(_) => {}
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Err(e) = execute(book, &line, &mut Printer) {
            print_chain(&e);
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// EXTENSIONS: things real engines have, in roughly increasing difficulty.
// Write a test first for each one.
//
//  1. Top of book. `fn quote(&self) -> Option<(Price, Qty, Price, Qty)>`:
//     best bid, its size, best ask, its size. None if either side is empty.
//  2. Cancel/replace. `fn replace(&mut self, id, new_price, new_qty, sink)`:
//     cancel + submit under the SAME trader and side, so it loses priority.
//     What should happen if the new order is invalid: is the old one still
//     cancelled? (Decide, then make the test say so.)
//  3. STP modes. Make the self-trade policy a field:
//     `enum Stp { CancelResting, CancelTaker, CancelBoth }`. Which one makes
//     `fillable` (the FOK check) wrong as written?
//  4. Market data feed. A second EventSink that only keeps what the PUBLIC
//     may see: trades and level changes, but no trader ids.
//  5. Stop orders. Hold them aside; when a trade prints at or through the
//     stop price, submit them as market orders. Watch out: submitting from
//     inside the matching loop needs `&mut self` twice.
//  6. Slab storage (phase 9). Keep orders in `Vec<Option<Resting>>`, let the
//     index map OrderId -> slot, and make each level a linked list of slots
//     (prev/next indexes). Cancel becomes O(1). Keep check() passing.
//  7. Many instruments. `HashMap<Symbol, Book>`, where the tick size is
//     per instrument.
//  8. Threads (phase 10). One thread owns the books and reads commands from
//     a channel. Why is one writer usually better than Arc<Mutex<Book>>?
// ═════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn lim(trader: u32, side: Side, qty: u64, price: i64) -> NewOrder {
        lim_tif(trader, side, qty, price, TimeInForce::Gtc)
    }

    fn lim_tif(trader: u32, side: Side, qty: u64, price: i64, tif: TimeInForce) -> NewOrder {
        NewOrder {
            trader: TraderId(trader),
            side,
            qty: Qty(qty),
            kind: OrderType::Limit {
                price: Price(price),
                tif,
            },
        }
    }

    fn mkt(trader: u32, side: Side, qty: u64) -> NewOrder {
        NewOrder {
            trader: TraderId(trader),
            side,
            qty: Qty(qty),
            kind: OrderType::Market,
        }
    }

    fn submit(book: &mut Book, o: NewOrder) -> Vec<Event> {
        let mut events = Vec::new();
        book.submit(o, &mut events).unwrap();
        events
    }

    /// (maker id, price, qty) of each trade, in order.
    fn trades(events: &[Event]) -> Vec<(u64, i64, u64)> {
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
    fn cancels(events: &[Event]) -> Vec<(u64, u64, CancelReason)> {
        events
            .iter()
            .filter_map(|e| match *e {
                Event::Cancelled { id, qty, reason } => Some((id.0, qty.0, reason)),
                _ => None,
            })
            .collect()
    }

    fn depth(book: &Book, side: Side) -> Vec<(i64, u64)> {
        book.depth(side, usize::MAX)
            .into_iter()
            .map(|(p, q)| (p.0, q.0))
            .collect()
    }

    /// ASK 1010: #3 t1 5
    /// ASK 1005: #1 t1 3, #2 t2 4
    /// BID 1000: #4 t3 6
    /// BID  995: #5 t4 2
    fn sample() -> Book {
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

    #[test]
    fn t_price_time_priority() {
        let mut b = sample();
        let ev = submit(&mut b, lim(9, Side::Buy, 5, 1005));
        assert_eq!(trades(&ev), vec![(1, 1005, 3), (2, 1005, 2)]); // #1 before #2
        assert_eq!(b.order(OrderId(2)).map(|r| r.qty), Some(Qty(2))); // partly filled, still first
        assert_eq!(b.best_ask(), Some(Price(1005)));
        assert!(b.order(OrderId(1)).is_none());
    }

    #[test]
    fn t_trades_at_maker_price_and_sweeps_levels() {
        let mut b = sample();
        let ev = submit(&mut b, lim(9, Side::Buy, 20, 1010));
        assert_eq!(trades(&ev), vec![(1, 1005, 3), (2, 1005, 4), (3, 1010, 5)]);
        assert!(ev.contains(&Event::Rested {
            id: OrderId(6),
            price: Price(1010),
            qty: Qty(8)
        }));
        assert!(b.asks.is_empty());
        assert_eq!(b.best_bid(), Some(Price(1010)));
        assert_eq!(b.spread(), None);
    }

    #[test]
    fn t_ioc_cancels_remainder() {
        let mut b = sample();
        let ev = submit(&mut b, lim_tif(9, Side::Sell, 10, 995, TimeInForce::Ioc));
        assert_eq!(trades(&ev), vec![(4, 1000, 6), (5, 995, 2)]);
        assert_eq!(cancels(&ev), vec![(6, 2, CancelReason::Unfilled)]);
        assert!(b.order(OrderId(6)).is_none());
        assert_eq!(b.best_bid(), None);
        assert_eq!(b.best_ask(), Some(Price(1005))); // other side untouched
    }

    #[test]
    fn t_market_orders() {
        let mut b = sample();
        let ev = submit(&mut b, mkt(9, Side::Buy, 100));
        assert_eq!(trades(&ev).len(), 3);
        assert_eq!(cancels(&ev), vec![(6, 88, CancelReason::Unfilled)]);
        assert!(b.asks.is_empty());

        let mut empty = Book::new(TICK);
        let ev = submit(&mut empty, mkt(9, Side::Sell, 5));
        assert_eq!(ev.len(), 2); // Accepted, then Cancelled
        assert_eq!(cancels(&ev), vec![(1, 5, CancelReason::Unfilled)]);
    }

    #[test]
    fn t_fill_or_kill() {
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
        assert!(b.asks.is_empty());
    }

    #[test]
    fn t_fok_ignores_own_liquidity() {
        let mut b = sample();
        // t1 owns #1 (3 @ 1005) and #3 (5 @ 1010); only #2's 4 are fillable for t1.
        let ev = submit(&mut b, lim_tif(1, Side::Buy, 5, 1010, TimeInForce::Fok));
        assert_eq!(cancels(&ev), vec![(6, 5, CancelReason::FillOrKill)]);
        assert_eq!(b.len(), 5);
    }

    #[test]
    fn t_post_only() {
        let mut b = sample();
        let ev = submit(
            &mut b,
            lim_tif(9, Side::Buy, 1, 1005, TimeInForce::PostOnly),
        );
        assert_eq!(cancels(&ev), vec![(6, 1, CancelReason::WouldCross)]);
        assert!(trades(&ev).is_empty());

        submit(
            &mut b,
            lim_tif(9, Side::Buy, 1, 1000, TimeInForce::PostOnly),
        );
        let queue: Vec<u64> = b.bids[&Price(1000)].orders.iter().map(|r| r.id.0).collect();
        assert_eq!(queue, vec![4, 7]); // behind #4
    }

    #[test]
    fn t_self_trade_prevention() {
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

    #[test]
    fn t_cancel() {
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
    fn t_amend_keeps_priority() {
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
    fn t_rejects_change_nothing() {
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
    fn t_market_data() {
        let b = sample();
        assert_eq!(depth(&b, Side::Buy), vec![(1000, 6), (995, 2)]);
        assert_eq!(b.depth(Side::Sell, 1), vec![(Price(1005), Qty(7))]);
        assert_eq!(b.spread(), Some(5));
        assert_eq!(b.len(), 5);
        assert!(Book::new(TICK).depth(Side::Buy, 3).is_empty());
    }

    #[test]
    fn t_parse() {
        assert_eq!(
            parse_command("t3 BUY 10 @ 1000"),
            Ok(Command::Submit(lim(3, Side::Buy, 10, 1000)))
        );
        assert_eq!(
            parse_command("  t1 sell 2 @ 995 ioc "),
            Ok(Command::Submit(lim_tif(
                1,
                Side::Sell,
                2,
                995,
                TimeInForce::Ioc
            )))
        );
        assert_eq!(
            parse_command("t2 sell 5 mkt"),
            Ok(Command::Submit(mkt(2, Side::Sell, 5)))
        );
        assert_eq!(parse_command("cancel #4"), Ok(Command::Cancel(OrderId(4))));
        assert_eq!(
            parse_command("amend 4 1"),
            Ok(Command::Amend(OrderId(4), Qty(1)))
        );
        assert_eq!(parse_command("book"), Ok(Command::Show));

        assert_eq!(parse_command("   "), Err(ParseError::Empty));
        assert!(matches!(
            parse_command("t1 hold 1 @ 5"),
            Err(ParseError::Unknown { what: "side", .. })
        ));
        assert!(matches!(
            parse_command("t1 buy 1 @ 5 gfd"),
            Err(ParseError::Unknown {
                what: "time in force",
                ..
            })
        ));
        assert!(matches!(
            parse_command("t1 buy 1 at 5"),
            Err(ParseError::Unknown {
                what: "order type",
                ..
            })
        ));
        assert!(matches!(
            parse_command("tx buy 1 @ 5"),
            Err(ParseError::BadNumber { what: "trader", .. })
        ));
        assert!(matches!(
            parse_command("t1 buy -1 @ 5"),
            Err(ParseError::BadNumber { what: "qty", .. })
        ));
        assert!(matches!(
            parse_command("flip the table"),
            Err(ParseError::Unknown {
                what: "command",
                ..
            })
        ));
    }

    #[test]
    fn t_execute_errors_chain() {
        let mut b = sample();
        let mut ev = Vec::new();
        let e = execute(&mut b, "t1 buy lots @ 1000", &mut ev).unwrap_err();
        assert!(matches!(
            e,
            EngineError::Parse(ParseError::BadNumber { what: "qty", .. })
        ));
        // EngineError -> ParseError -> ParseIntError
        assert!(e.source().and_then(|p| p.source()).is_some());

        let e = execute(&mut b, "cancel 99", &mut ev).unwrap_err();
        assert_eq!(e, EngineError::Reject(Reject::UnknownOrder(OrderId(99))));
        execute(&mut b, "t9 buy 1 @ 1005", &mut ev).unwrap();
        assert_eq!(trades(&ev), vec![(1, 1005, 1)]);
    }

    /// Thousands of random operations; check() runs after each one (inside
    /// the methods, through debug_assert!) and once more here.
    #[test]
    fn t_random_operations_keep_invariants() {
        // A tiny deterministic PRNG, so a failure always reproduces.
        struct Lcg(u64);
        impl Lcg {
            fn below(&mut self, n: u64) -> u64 {
                self.0 = self
                    .0
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (self.0 >> 33) % n
            }
        }

        let mut rng = Lcg(42);
        let mut b = Book::new(TICK);
        let mut ev = Vec::new();
        let mut sent = 0;
        for _ in 0..5_000 {
            match rng.below(10) {
                0..=5 => {
                    let side = if rng.below(2) == 0 {
                        Side::Buy
                    } else {
                        Side::Sell
                    };
                    let trader = TraderId(rng.below(4) as u32);
                    let qty = Qty(1 + rng.below(10));
                    let price = Price(950 + 5 * rng.below(21) as i64);
                    let tif = [
                        TimeInForce::Gtc,
                        TimeInForce::Gtc,
                        TimeInForce::Ioc,
                        TimeInForce::Fok,
                        TimeInForce::PostOnly,
                    ][rng.below(5) as usize];
                    let kind = if rng.below(10) == 0 {
                        OrderType::Market
                    } else {
                        OrderType::Limit { price, tif }
                    };
                    sent += qty.0;
                    b.submit(
                        NewOrder {
                            trader,
                            side,
                            qty,
                            kind,
                        },
                        &mut ev,
                    )
                    .unwrap();
                }
                6..=8 => {
                    let _ = b.cancel(OrderId(1 + rng.below(b.next_id)), &mut ev);
                }
                _ => {
                    let _ = b.amend(
                        OrderId(1 + rng.below(b.next_id)),
                        Qty(1 + rng.below(5)),
                        &mut ev,
                    );
                }
            }
            assert_eq!(b.check(), Ok(()));
        }
        let traded: u64 = trades(&ev).iter().map(|t| t.2).sum();
        assert!(traded > 0 && traded <= sent);
        assert!(b.len() > 0);
    }
}
