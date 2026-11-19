//! PHASE 2 — Rust's type system
//!
//!   cargo run  --bin p02_types
//!   cargo test --bin p02_types
//!
//! TypeScript → Rust cheat sheet:
//!
//!   interface / object type    →  struct
//!   discriminated union        →  enum (with data in its variants)
//!   T | null | undefined       →  Option<T>
//!   throw / Promise.reject     →  Result<T, E>
//!   interface (behaviour)      →  trait
//!   <T extends Foo>            →  <T: Foo>  (trait bound)
//!   branded types              →  tuple-struct "newtype"
//!
//! Everything here is a piece of your future order-book exchange.

use std::fmt;

fn main() {
    section_structs();
    section_newtypes();
    section_enums();
    section_impl_methods();
    section_match_and_destructuring();
    section_option();
    section_result();
    section_generics();
    section_traits();
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Structs ≈ TS interfaces, except they are real data layouts
// ─────────────────────────────────────────────────────────────────────────────

// `#[derive(...)]` asks the compiler to generate trait implementations:
//   Debug     -> lets you print with {:?}  (otherwise you can't print it at all!)
//   Clone     -> .clone() makes a copy (phase 3 explains why this is opt-in)
//   PartialEq -> lets you use == (Rust has no default structural equality)
#[derive(Debug, Clone, PartialEq)]
struct Trade {
    id: u64,
    price_ticks: i64,
    qty: u64,
    aggressor_is_buy: bool,
}

fn section_structs() {
    println!("\n== structs ==");

    // You must set EVERY field. There is no `undefined`, and no optional
    // fields unless you write Option<T> (see below).
    let t = Trade {
        id: 1,
        price_ticks: 10_050,
        qty: 3,
        aggressor_is_buy: true,
    };
    println!("{t:?}"); // Debug, one line
    println!("{t:#?}"); // Debug, pretty-printed

    // Field-init shorthand, like `{ id, qty }` in JS:
    let id = 2;
    let qty = 7;
    // Struct update syntax, like `{ ...t, id, qty }`. Note that `..t` goes LAST.
    let t2 = Trade {
        id,
        qty,
        ..t.clone()
    };
    println!("t2 = {t2:?}");
    println!("t == t2 ? {}", t == t2);

    // ❌ UNCOMMENT -> error[E0063]: missing field `aggressor_is_buy`
    // let bad = Trade { id: 3, price_ticks: 1, qty: 1 };

    // ❌ UNCOMMENT -> error[E0594]: cannot assign to `t.qty`, as `t` is not declared as mutable
    // t.qty = 10;
    // Mutability belongs to the BINDING, not to individual fields.
    // `let mut t = ...` makes all of its fields mutable.
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Tuple structs and the newtype pattern
// ─────────────────────────────────────────────────────────────────────────────
//
// The bug: `fn place(price: i64, qty: i64)` and you call place(qty, price).
// Both are i64 so it compiles. In TS you'd use branded types
// (`number & { __brand: 'Price' }`). In Rust a one-field tuple struct makes a
// genuinely distinct type at ZERO runtime cost: Price is just an i64 in memory.

// Copy: copying is implicit, as with integers (phase 3 explains Copy vs Clone).
// PartialOrd/Ord: enables < > comparisons. Eq/Hash: usable as a HashMap key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Price(i64); // in ticks

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Qty(u64); // in lots

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct OrderId(u64);

fn notional(price: Price, qty: Qty) -> i64 {
    price.0 * qty.0 as i64 // .0 reaches the inner value
}

fn section_newtypes() {
    println!("\n== newtypes ==");
    let p = Price(10_050);
    let q = Qty(4);
    println!("notional = {}", notional(p, q));
    println!("Price(1) < Price(2) ? {}", Price(1) < Price(2));

    // ❌ UNCOMMENT -> error[E0308]: arguments to this function are incorrect
    //   (expected `Price`, found `Qty`, and the other way round)
    // notional(q, p);
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Enums are discriminated unions
// ─────────────────────────────────────────────────────────────────────────────
//
// TS:
//   type OrderType =
//     | { kind: 'market' }
//     | { kind: 'limit';     price: number }
//     | { kind: 'stopLimit'; stop: number; limit: number };
//
// Rust: the "kind" tag is built in, and each variant carries its own data.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OrderType {
    Market,                 // unit variant (no data)
    Limit { price: Price }, // struct-like variant
    StopLimit { stop: Price, limit: Price },
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)] // Fok is only used in patterns, never constructed
enum TimeInForce {
    Gtc, // good-til-cancelled: rest on the book
    Ioc, // immediate-or-cancel: fill what you can, cancel the rest
    Fok, // fill-or-kill: fill everything immediately or nothing
}

// Variants can also be tuple-like. This is how you'd model what an exchange
// emits, one variant per event type:
#[derive(Debug)]
#[allow(dead_code)]
enum ExchangeEvent {
    Accepted(OrderId),
    Rejected(OrderId, String),
    Fill {
        maker: OrderId,
        taker: OrderId,
        price: Price,
        qty: Qty,
    },
    Cancelled(OrderId),
}

fn section_enums() {
    println!("\n== enums ==");
    let ot = OrderType::Limit {
        price: Price(10_000),
    };
    let ev = ExchangeEvent::Fill {
        maker: OrderId(1),
        taker: OrderId(2),
        price: Price(10_000),
        qty: Qty(5),
    };
    println!("{ot:?} / {:?} / {ev:?}", TimeInForce::Ioc);

    // Rust doesn't let you read `ot.price` directly, because that field only
    // exists in one variant. You have to `match` on it (section 5). The same
    // rule is why TS makes you narrow a union before touching its fields,
    // except Rust has no way around it.
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. impl blocks: methods and associated functions
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Order {
    id: OrderId,
    side: Side,
    kind: OrderType,
    tif: TimeInForce,
    qty: Qty,
    filled: Qty,
}

impl Order {
    // ASSOCIATED FUNCTION: there's no `self`, so call it as Order::limit(...).
    // This is like a static method. Rust has no constructors; by convention
    // you write `new` or a descriptively named function.
    fn limit(id: u64, side: Side, price: i64, qty: u64) -> Self {
        // `Self` is an alias for the type the impl is for (Order)
        Order {
            id: OrderId(id),
            side,
            kind: OrderType::Limit {
                price: Price(price),
            },
            tif: TimeInForce::Gtc,
            qty: Qty(qty),
            filled: Qty(0),
        }
    }

    // METHOD taking `&self`: read-only access (phase 4: a shared borrow).
    fn remaining(&self) -> Qty {
        Qty(self.qty.0 - self.filled.0)
    }

    fn is_done(&self) -> bool {
        self.remaining().0 == 0
    }

    // METHOD taking `&mut self`: may modify the order. The caller's
    // binding must be `mut`.
    fn apply_fill(&mut self, qty: Qty) {
        // Note: in a real engine you'd validate qty <= remaining (see Result below).
        self.filled = Qty(self.filled.0 + qty.0);
    }

    // METHOD taking `self` (no &): CONSUMES the order. After the call the
    // caller can't use it any more. Phase 3 explains why; for now just note
    // that the three receiver kinds exist.
    fn into_id(self) -> OrderId {
        self.id
    }
}

fn section_impl_methods() {
    println!("\n== impl ==");
    let mut o = Order::limit(1, Side::Buy, 10_000, 10);
    o.apply_fill(Qty(4)); // Rust auto-references: same as (&mut o).apply_fill(...)
    println!("remaining={:?} done={}", o.remaining(), o.is_done());
    o.apply_fill(Qty(6));
    println!("remaining={:?} done={}", o.remaining(), o.is_done());
    let id = o.into_id();
    println!("consumed into {id:?}");

    // ❌ UNCOMMENT -> error[E0382]: borrow of moved value: `o`
    // println!("{:?}", o);
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. match + destructuring: the payoff for using enums
// ─────────────────────────────────────────────────────────────────────────────

fn describe(kind: &OrderType) -> String {
    match kind {
        OrderType::Market => "MKT".to_string(),
        // Pull fields out by name:
        OrderType::Limit { price } => format!("LMT @ {}", price.0),
        // Rename while destructuring (`stop: s`), or nest patterns
        // (`Price(l)` unwraps the newtype in place):
        OrderType::StopLimit {
            stop: s,
            limit: Price(l),
        } => {
            format!("STP {} -> LMT {}", s.0, l)
        }
    }
    // If you add a new OrderType variant later, EVERY match that doesn't
    // handle it becomes a compile error. That's the main reason to model
    // your domain with enums: the compiler finds every place to update.
    // (In TS this needs the `assertNever(x: never)` trick.)
}

fn section_match_and_destructuring() {
    println!("\n== match / destructuring ==");
    for k in [
        OrderType::Market,
        OrderType::Limit { price: Price(101) },
        OrderType::StopLimit {
            stop: Price(99),
            limit: Price(98),
        },
    ] {
        println!("{}", describe(&k));
    }

    // Matching on a tuple of values combines conditions neatly.
    let o = Order::limit(7, Side::Sell, 10_100, 5);
    let action = match (o.side, o.tif) {
        (Side::Buy, TimeInForce::Gtc) => "rest bid",
        (Side::Sell, TimeInForce::Gtc) => "rest ask",
        (_, TimeInForce::Ioc | TimeInForce::Fok) => "take or cancel",
    };
    println!("action = {action}");

    // Destructure a struct in a `let`. `..` ignores the remaining fields.
    let Order { id, side, .. } = &o;
    println!("id={id:?} side={side:?}");

    // `if let` handles a single pattern when you don't care about the others.
    if let OrderType::Limit { price } = o.kind {
        println!("limit price {}", price.0);
    }

    // `let ... else` destructures or bails out early, and keeps the happy
    // path flat. It's very common in request handlers.
    let OrderType::Limit { price } = o.kind else {
        println!("not a limit order");
        return;
    };
    println!("let-else got {}", price.0);
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Option<T>: there is no null
// ─────────────────────────────────────────────────────────────────────────────
//
// Option is just an enum in the standard library:
//     enum Option<T> { Some(T), None }
// A `Price` is ALWAYS a valid price. "Maybe a price" is a different type,
// `Option<Price>`, and the compiler makes you handle the None case before
// you can get at the value.

fn best_bid(bids: &[Price]) -> Option<Price> {
    // An empty book has no best bid. Return None; don't use 0 or -1 as a
    // sentinel value.
    bids.iter().max().copied()
}

fn spread(bids: &[Price], asks: &[Price]) -> Option<i64> {
    // `?` on an Option: if it's None, return None from this function right
    // away. Otherwise unwrap the value. It's like `?.` chaining but for
    // whole statements.
    let bid = best_bid(bids)?;
    let ask = asks.iter().min()?;
    Some(ask.0 - bid.0)
}

fn section_option() {
    println!("\n== Option ==");
    let bids = [Price(99), Price(100), Price(98)];
    let empty: [Price; 0] = [];

    println!("best bid = {:?}", best_bid(&bids));
    println!("best bid (empty) = {:?}", best_bid(&empty));

    // Ways to get at the value, roughly safest first:
    match best_bid(&bids) {
        Some(p) => println!("match: {}", p.0),
        None => println!("match: book empty"),
    }
    let p = best_bid(&empty).unwrap_or(Price(0)); // default value
    let shown = best_bid(&bids).map(|p| p.0 * 2); // transform inside (like ?.)
    println!("unwrap_or={p:?} map={shown:?}");
    println!("spread = {:?}", spread(&bids, &[Price(101), Price(102)]));
    println!("spread (no asks) = {:?}", spread(&bids, &[]));

    // .unwrap() PANICS on None. That's fine in tests and prototypes, but in
    // an exchange it brings the whole process down. Search for unwrap()
    // before you ship.

    // ❌ UNCOMMENT -> error[E0369]: cannot add `{integer}` to `Option<Price>`
    //   The compiler won't let you ignore the None case:
    // let x = best_bid(&bids) + 1;
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Result<T, E>: errors are values
// ─────────────────────────────────────────────────────────────────────────────
//
//     enum Result<T, E> { Ok(T), Err(E) }
//
// Compare TS `function validate(o): Order  // might throw ¯\_(ツ)_/¯`.
// In Rust the failure modes are in the SIGNATURE, as an enum the caller can
// match on. Phase 7 goes deeper (custom Error impls, `?` conversions, thiserror).

#[derive(Debug, PartialEq)]
enum OrderError {
    ZeroQty,
    PriceNotOnTick { price: i64, tick: i64 },
    MarketOrderCannotRest,
}

fn validate(o: &Order, tick: i64) -> Result<(), OrderError> {
    if o.qty.0 == 0 {
        return Err(OrderError::ZeroQty);
    }
    if let OrderType::Limit { price } = o.kind {
        if price.0 % tick != 0 {
            return Err(OrderError::PriceNotOnTick {
                price: price.0,
                tick,
            });
        }
    }
    if o.kind == OrderType::Market && o.tif == TimeInForce::Gtc {
        return Err(OrderError::MarketOrderCannotRest);
    }
    Ok(()) // success with no payload: "Ok(unit)"
}

fn section_result() {
    println!("\n== Result ==");
    let good = Order::limit(1, Side::Buy, 10_000, 5);
    let bad_qty = Order::limit(2, Side::Buy, 10_000, 0);
    let bad_tick = Order::limit(3, Side::Sell, 10_003, 5);

    for o in [&good, &bad_qty, &bad_tick] {
        match validate(o, 5) {
            Ok(()) => println!("order {:?}: ok", o.id),
            Err(OrderError::PriceNotOnTick { price, tick }) => {
                println!("order {:?}: {price} is not a multiple of {tick}", o.id)
            }
            Err(e) => println!("order {:?}: rejected {e:?}", o.id),
        }
    }

    // Results are #[must_use]. Ignoring one gives a warning:
    // ❌ UNCOMMENT -> warning: unused `Result` that must be used
    // validate(&good, 5);
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Generics
// ─────────────────────────────────────────────────────────────────────────────
//
// Like TS generics, with one big difference. Rust MONOMORPHIZES: it
// compiles a separate copy of the function for each concrete T you use. So
// generics cost nothing at runtime (no boxing, no type erasure), at the
// price of longer compile times and bigger binaries.

// A price level holding any kind of item. Order books keep a queue per
// price; generic makes it reusable/testable.
#[derive(Debug)]
struct Level<T> {
    price: Price,
    items: Vec<T>,
}

impl<T> Level<T> {
    fn new(price: Price) -> Self {
        Level {
            price,
            items: Vec::new(),
        }
    }
    fn push(&mut self, item: T) {
        self.items.push(item);
    }
    fn len(&self) -> usize {
        self.items.len()
    }
}

// A generic function needs BOUNDS to do anything with T. Without
// `PartialOrd`, the compiler doesn't know `>` makes sense for T. TS would
// just let you try; Rust requires you to state the capability.
fn largest<T: PartialOrd + Copy>(xs: &[T]) -> Option<T> {
    let mut best = *xs.first()?;
    for &x in xs {
        if x > best {
            best = x;
        }
    }
    Some(best)
}

fn section_generics() {
    println!("\n== generics ==");
    let mut lvl: Level<OrderId> = Level::new(Price(100));
    lvl.push(OrderId(1));
    lvl.push(OrderId(2));
    println!(
        "level @ {} has {} orders: {:?}",
        lvl.price.0,
        lvl.len(),
        lvl.items
    );

    println!("largest i64   = {:?}", largest(&[3, 9, 2]));
    println!("largest Price = {:?}", largest(&[Price(5), Price(7)]));

    // ❌ UNCOMMENT -> error[E0369]: binary operation `>` cannot be applied to type `T`
    //   (delete the `T: PartialOrd + Copy` bounds above, keep just <T>)
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Traits ≈ interfaces for behaviour
// ─────────────────────────────────────────────────────────────────────────────
//
// Differences from TS interfaces:
//   - NOMINAL, not structural. A type implements a trait only if you write
//     `impl Trait for Type`. Having the right methods isn't enough.
//   - You can implement YOUR trait for SOMEONE ELSE's type (even i64), and
//     a std trait (Display) for YOUR type.
//   - Traits can have default method bodies.

trait FeeSchedule {
    /// Fee in ticks for a given notional. Negative = rebate.
    fn fee(&self, notional: i64, is_maker: bool) -> i64;

    // Default method: implementors get this for free and may override it.
    fn name(&self) -> String {
        "unnamed".to_string()
    }
}

struct FlatBps {
    maker_bps: i64,
    taker_bps: i64,
}

impl FeeSchedule for FlatBps {
    fn fee(&self, notional: i64, is_maker: bool) -> i64 {
        let bps = if is_maker {
            self.maker_bps
        } else {
            self.taker_bps
        };
        notional * bps / 10_000
    }
    fn name(&self) -> String {
        format!("flat {}/{} bps", self.maker_bps, self.taker_bps)
    }
}

struct ZeroFee;

impl FeeSchedule for ZeroFee {
    fn fee(&self, _notional: i64, _is_maker: bool) -> i64 {
        0
    }
    // uses the default `name`
}

// TRAIT BOUND: works for any F that implements FeeSchedule.
// The two spellings below mean the same thing:
//   fn total_fees<F: FeeSchedule>(f: &F, ...)
//   fn total_fees(f: &impl FeeSchedule, ...)
// For long bound lists there's also `where F: FeeSchedule + Clone`.
fn total_fees<F: FeeSchedule>(sched: &F, fills: &[(i64, bool)]) -> i64 {
    fills.iter().map(|&(n, maker)| sched.fee(n, maker)).sum()
}

// Implementing a std trait: Display controls how `{}` formats your type.
// Debug ({:?}) is for developers; Display ({}) is for users and logs.
impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}

fn section_traits() {
    println!("\n== traits ==");
    let fills = [(1_000_000, true), (500_000, false)];
    let flat = FlatBps {
        maker_bps: -1,
        taker_bps: 5,
    };
    println!("{}: {}", flat.name(), total_fees(&flat, &fills));
    println!("{}: {}", ZeroFee.name(), total_fees(&ZeroFee, &fills));
    println!("Display price: {}", Price(6_500_025));

    // ❌ UNCOMMENT -> error[E0277]: `Qty` doesn't implement `std::fmt::Display`
    // println!("{}", Qty(5));
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES: `cargo test --bin p02_types`
// ═════════════════════════════════════════════════════════════════════════════

impl Side {
    /// Buy <-> Sell.
    #[allow(dead_code)]
    fn opposite(self) -> Side {
        if self == Side::Buy {
            Side::Sell
        } else {
            Side::Buy
        }
    }
}

/// Can this order trade immediately against the best price on the OTHER side?
///   - Market: marketable if `best_opposite` exists
///   - Limit Buy:  marketable if best ask <= limit price
///   - Limit Sell: marketable if best bid >= limit price
///   - StopLimit: never (treat as not yet triggered)
///   - If `best_opposite` is None, nothing is marketable.
#[allow(dead_code)]
fn is_marketable(o: &Order, best_opposite: Option<Price>) -> bool {
    //  let best_price = match best_opposite {
    //     None => return false,
    //     Some(best) => best,
    // };

    let Some(best_price) = best_opposite else {
        return false;
    };

    match o.kind {
        (OrderType::Limit { price }) => {
            if o.side == Side::Buy {
                price >= best_price
            } else {
                price <= best_price
            }
        }
        (OrderType::Market) => true,
        (OrderType::StopLimit { stop, limit }) => false,
    }
}

/// Apply a fill but REFUSE overfills: return Err(FillError::Overfill { remaining })
/// if `qty` > remaining, and Err(FillError::ZeroQty) if qty is 0. On success
/// update `filled` and return the new remaining qty.
#[derive(Debug, PartialEq)]
#[allow(dead_code)]
enum FillError {
    ZeroQty,
    Overfill { remaining: Qty },
}

#[allow(dead_code)]
fn try_fill(o: &mut Order, qty: Qty) -> Result<Qty, FillError> {
    if qty.0 == 0 {
        return Err(FillError::ZeroQty);
    }

    if qty > o.remaining() {
        return Err(FillError::Overfill {
            remaining: o.remaining(),
        });
    }

    o.apply_fill(qty);
    Ok(o.remaining())
}

/// Implement Display for Side so that `format!("{}", Side::Buy) == "BUY"`.
/// (Write the `impl fmt::Display for Side` block yourself, mirroring Price above,
///  then delete the #[ignore] on the test below.)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_opposite() {
        assert_eq!(Side::Buy.opposite(), Side::Sell);
        assert_eq!(Side::Sell.opposite(), Side::Buy);
    }

    #[test]
    fn t_is_marketable() {
        let buy = Order::limit(1, Side::Buy, 100, 1);
        let sell = Order::limit(2, Side::Sell, 100, 1);
        let mut mkt = Order::limit(3, Side::Buy, 0, 1);
        mkt.kind = OrderType::Market;

        assert!(is_marketable(&buy, Some(Price(100))));
        assert!(!is_marketable(&buy, Some(Price(101))));
        assert!(is_marketable(&sell, Some(Price(101))));
        assert!(!is_marketable(&sell, Some(Price(99))));
        assert!(is_marketable(&mkt, Some(Price(1))));
        assert!(!is_marketable(&mkt, None));
        assert!(!is_marketable(&buy, None));
    }

    #[test]
    fn t_try_fill() {
        let mut o = Order::limit(1, Side::Buy, 100, 10);
        assert_eq!(try_fill(&mut o, Qty(0)), Err(FillError::ZeroQty));
        assert_eq!(try_fill(&mut o, Qty(4)), Ok(Qty(6)));
        assert_eq!(
            try_fill(&mut o, Qty(7)),
            Err(FillError::Overfill { remaining: Qty(6) })
        );
        assert_eq!(try_fill(&mut o, Qty(6)), Ok(Qty(0)));
        assert!(o.is_done());
    }

    #[test]
    #[ignore = "write `impl fmt::Display for Side` first, then remove this line"]
    fn t_display_side() {
        // Uncomment once your impl exists:
        // assert_eq!(format!("{}", Side::Buy), "BUY");
        // assert_eq!(format!("{}", Side::Sell), "SELL");
    }
}
