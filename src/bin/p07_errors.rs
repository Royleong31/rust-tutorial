//! PHASE 7 — Error handling
//!
//!   cargo run  --bin p07_errors
//!   cargo test --bin p07_errors
//!
//! Phase 2 introduced Option and Result. This phase is about using them in
//! real code: passing errors up with `?`, designing your own error types,
//! and deciding when a failure should be a value and when it should crash.
//!
//!   TS:    try { await placeOrder() } catch (e) { /* e: unknown */ }
//!   Rust:  place_order() -> Result<OrderAck, OrderError>
//!
//! There are two kinds of failure:
//!
//!   RECOVERABLE    Something you knew could happen: bad input, not enough
//!                  balance, a timed-out connection.
//!                  -> return a Result, and let the caller decide what to do.
//!
//!   UNRECOVERABLE  A bug: something your own code was supposed to guarantee
//!                  turned out false.
//!                  -> panic!. Carrying on would only make things worse.
//!
//! In an exchange a rejected order is NOT a crash-worthy error. It's a normal
//! outcome you send back to the client. A panic in the matching thread takes
//! the market down for everyone.

use std::error::Error;
use std::fmt;
use std::num::ParseIntError;

fn main() {
    section_panic();
    section_handling_results();
    section_question_mark();
    section_custom_errors();
    section_layered_errors();
    section_box_dyn_error();
    section_validate_at_the_boundary();
    section_errors_in_iterators();
    puzzles();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq)]
struct NewOrder {
    side: Side,
    symbol: String,
    qty: u64,
    price: i64,
}

const TICK: i64 = 5;
const SYMBOLS: [&str; 2] = ["BTC-USD", "ETH-USD"];

// ─────────────────────────────────────────────────────────────────────────────
// 1. panic!: for bugs, not for bad input
// ─────────────────────────────────────────────────────────────────────────────

fn section_panic() {
    println!("\n== panic ==");

    // A panic stops the current thread. It prints a message, unwinds the
    // stack (running Drop for everything on the way, as in phase 3), and if
    // it's the main thread, the program exits with code 101.
    //
    // Things that panic: panic!(), unwrap() on None or Err, expect(), an
    // index out of bounds, integer overflow in debug builds, dividing an
    // integer by zero, todo!(), unreachable!().

    // catch_unwind can stop a panic at a boundary, such as a thread pool or
    // a call into C code. It is NOT try/catch for ordinary errors. It's used
    // here only so the demo keeps running.
    println!("  (the 'panicked at' message below is expected)");
    let result = std::panic::catch_unwind(|| {
        let levels: Vec<i64> = Vec::new();
        levels[0]
    });
    println!("  caught the panic: {}", result.is_err());

    // unwrap() and expect() both panic on None/Err. expect() lets you say WHY
    // you thought it couldn't fail, which is what you want to read in a crash log.
    let tick: i64 = "5".parse().expect("tick size in config must be an integer");
    println!("  tick = {tick}");

    // Good reasons to panic:
    //   - tests and prototypes
    //   - a broken INVARIANT. Example: the index says order 7 rests at 101,
    //     but level 101 doesn't contain it. Your own code is wrong, and
    //     carrying on would corrupt the book.
    //   - startup: a missing config file. Crashing right away with a clear
    //     message beats running half-configured.
    // Bad reason: anything a client or the network can trigger.
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Handling a Result
// ─────────────────────────────────────────────────────────────────────────────

fn parse_qty(s: &str) -> Result<u64, ParseIntError> {
    s.trim().parse::<u64>()
}

fn section_handling_results() {
    println!("\n== handling a Result ==");

    // match: handle both cases explicitly.
    match parse_qty("25") {
        Ok(q) => println!("  match: qty {q}"),
        Err(e) => println!("  match: bad qty: {e}"),
    }

    // if let, when you only care about one side:
    if let Err(e) = parse_qty("abc") {
        println!("  if let: {e}"); // ParseIntError's Display text
    }

    // Fallback values:
    let a = parse_qty("x").unwrap_or(1); // a fixed default
    let b = parse_qty("").unwrap_or_default(); // the type's default (0 for numbers)
    let c = parse_qty("x").unwrap_or_else(|e| {
        println!("  unwrap_or_else saw: {e}"); // runs only on Err, and gets the error
        2
    });
    println!("  fallbacks: {a} {b} {c}");

    // Transform without unwrapping, like the Option methods from phase 2:
    let lots = parse_qty("7").map(|q| q * 100); // changes the Ok value
    let msg = parse_qty("x").map_err(|e| format!("qty: {e}")); // changes the Err value
    let positive = parse_qty("0").map_err(|e| e.to_string()).and_then(|q| {
        if q > 0 {
            Ok(q)
        } else {
            Err("qty must be > 0".to_string())
        }
    }); // chain another check
    println!("  map={lots:?} map_err={msg:?} and_then={positive:?}");

    // Converting between Option and Result:
    //   result.ok()           Result<T, E> -> Option<T>   (drops the error)
    //   option.ok_or(e)       Option<T> -> Result<T, E>
    //   option.ok_or_else(f)  same, but only builds the error when needed
    let best_bid: Option<i64> = None;
    let r: Result<i64, &str> = best_bid.ok_or("book is empty");
    println!("  ok_or: {r:?}   ok(): {:?}", parse_qty("9").ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. The `?` operator
// ─────────────────────────────────────────────────────────────────────────────
//
// `?` on a Result means:
//   Ok(v)   -> the expression evaluates to v, and the function carries on
//   Err(e)  -> RETURN Err(From::from(e)) from the current function, right now
//
// So this line:
//     let q = parse_qty(s)?;
// is short for:
//     let q = match parse_qty(s) {
//         Ok(v) => v,
//         Err(e) => return Err(From::from(e)),   // From is section 5
//     };
//
// It's TS's "let it throw to the caller", except you can SEE every place it
// happens, and the signature says which errors can come out.

fn notional(qty: &str, price: &str) -> Result<i64, ParseIntError> {
    let q: i64 = qty.parse()?;
    let p: i64 = price.parse()?;
    Ok(q * p) // the success value must be wrapped in Ok
}

fn section_question_mark() {
    println!("\n== the ? operator ==");
    println!("  notional(5, 100) = {:?}", notional("5", "100"));
    println!("  notional(5, abc) = {:?}", notional("5", "abc"));

    // `?` needs a function that returns Result (or Option). This one returns ():
    // ❌ UNCOMMENT -> error[E0277]: the `?` operator can only be used in a function that returns `Result` or `Option`
    // let q: u64 = "5".parse()?;

    // You can't mix them either. `?` on an Option inside a function that
    // returns Result doesn't compile (PUZZLE 2). Convert it with ok_or first.
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Your own error type
// ─────────────────────────────────────────────────────────────────────────────
//
// An enum, one variant per way things can go wrong, carrying the details.
// Callers can `match` on it, which a string or a TS `Error` can't offer.

#[derive(Debug, PartialEq)]
enum OrderError {
    ZeroQty,
    PriceOffTick { price: i64, tick: i64 },
    UnknownSymbol(String),
    InsufficientBalance { needed: i64, available: i64 },
}

// Display is the human-readable message, the one you'd send to a client.
// Debug ({:?}) stays the developer view for logs.
impl fmt::Display for OrderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OrderError::ZeroQty => write!(f, "quantity must be greater than zero"),
            OrderError::PriceOffTick { price, tick } => {
                write!(f, "price {price} is not a multiple of the tick size {tick}")
            }
            OrderError::UnknownSymbol(s) => write!(f, "unknown symbol {s}"),
            OrderError::InsufficientBalance { needed, available } => {
                write!(f, "insufficient balance: need {needed}, have {available}")
            }
        }
    }
}

// The standard Error trait. It needs Debug + Display, and its methods all
// have defaults, so an empty impl is enough. It lets your error go into a
// Box<dyn Error> (section 6) and join error chains (section 5).
impl Error for OrderError {}

fn validate(o: &NewOrder, balance: i64) -> Result<(), OrderError> {
    if o.qty == 0 {
        return Err(OrderError::ZeroQty);
    }
    if o.price % TICK != 0 {
        return Err(OrderError::PriceOffTick {
            price: o.price,
            tick: TICK,
        });
    }
    if !SYMBOLS.contains(&o.symbol.as_str()) {
        return Err(OrderError::UnknownSymbol(o.symbol.clone()));
    }
    let needed = o.price * o.qty as i64;
    if o.side == Side::Buy && needed > balance {
        return Err(OrderError::InsufficientBalance {
            needed,
            available: balance,
        });
    }
    Ok(())
}

// Because the error is an enum, mapping it to a protocol reject code is a
// match the compiler checks for completeness. Add a variant and this stops
// compiling until you give it a code.
fn reject_code(e: &OrderError) -> u32 {
    match e {
        OrderError::UnknownSymbol(_) => 1,
        OrderError::InsufficientBalance { .. } => 3,
        OrderError::ZeroQty | OrderError::PriceOffTick { .. } => 5,
    }
}

fn section_custom_errors() {
    println!("\n== custom error types ==");
    let orders = [
        NewOrder {
            side: Side::Buy,
            symbol: "BTC-USD".into(),
            qty: 2,
            price: 100,
        },
        NewOrder {
            side: Side::Buy,
            symbol: "BTC-USD".into(),
            qty: 2,
            price: 102,
        },
        NewOrder {
            side: Side::Buy,
            symbol: "DOGE-USD".into(),
            qty: 2,
            price: 100,
        },
        NewOrder {
            side: Side::Buy,
            symbol: "BTC-USD".into(),
            qty: 50,
            price: 100,
        },
    ];
    for o in &orders {
        match validate(o, 1_000) {
            Ok(()) => println!("  accepted {} {} @ {}", o.qty, o.symbol, o.price),
            Err(e) => println!("  reject code {}: {e}", reject_code(&e)),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Layers of errors, and how `?` converts between them
// ─────────────────────────────────────────────────────────────────────────────
//
// Real code has layers: parsing, validation, risk, the engine. Each layer has
// its own error type, and the outer layer wraps the inner ones:
//
//     EngineError ── Parse(ParseError) ── BadNumber { source: ParseIntError }
//                 └─ Order(OrderError)
//
// The trick is that `?` calls From::from on the error. If you write
// `impl From<ParseError> for EngineError`, then `?` on a Result<_, ParseError>
// inside a function returning Result<_, EngineError> converts automatically.

#[derive(Debug, PartialEq)]
enum ParseError {
    MissingField(&'static str),
    BadSide(String),
    BadNumber {
        field: &'static str,
        source: ParseIntError,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::MissingField(name) => write!(f, "missing field: {name}"),
            ParseError::BadSide(s) => write!(f, "bad side {s:?} (want BUY or SELL)"),
            ParseError::BadNumber { field, .. } => write!(f, "{field} is not a valid number"),
        }
    }
}

// This time Error::source is overridden: it returns the error that CAUSED
// this one, so a logger can print the whole chain.
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
    Order(OrderError),
    #[allow(dead_code)] // only built in the exercises
    Risk(RiskError),
}

// A wrapping error should EITHER include the inner message in its Display
// OR return the inner error from source(), not both. Otherwise a chain
// printer shows every message twice. Here the inner errors go in source(),
// so Display only describes this layer.
impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EngineError::Parse(_) => write!(f, "could not parse order"),
            EngineError::Order(_) => write!(f, "order rejected"),
            EngineError::Risk(e) => write!(f, "risk check failed: {e:?}"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EngineError::Parse(e) => Some(e),
            EngineError::Order(e) => Some(e),
            EngineError::Risk(_) => None,
        }
    }
}

impl From<ParseError> for EngineError {
    fn from(e: ParseError) -> Self {
        EngineError::Parse(e)
    }
}

impl From<OrderError> for EngineError {
    fn from(e: OrderError) -> Self {
        EngineError::Order(e)
    }
}

/// "BUY BTC-USD 5 100" -> NewOrder
fn parse_line(line: &str) -> Result<NewOrder, ParseError> {
    let mut parts = line.split_whitespace();

    // ok_or turns "no next word" (None) into our error, then `?` returns it.
    let side = match parts.next().ok_or(ParseError::MissingField("side"))? {
        "BUY" => Side::Buy,
        "SELL" => Side::Sell,
        other => return Err(ParseError::BadSide(other.to_string())),
    };
    let symbol = parts
        .next()
        .ok_or(ParseError::MissingField("symbol"))?
        .to_string();

    // map_err adds CONTEXT (which field?) and keeps the original error as the source.
    let qty = parts
        .next()
        .ok_or(ParseError::MissingField("qty"))?
        .parse()
        .map_err(|e| ParseError::BadNumber {
            field: "qty",
            source: e,
        })?;
    let price = parts
        .next()
        .ok_or(ParseError::MissingField("price"))?
        .parse()
        .map_err(|e| ParseError::BadNumber {
            field: "price",
            source: e,
        })?;

    Ok(NewOrder {
        side,
        symbol,
        qty,
        price,
    })
}

// Two different error types come out of the two calls, and `?` converts
// both into EngineError through the From impls above.
fn submit(line: &str, balance: i64) -> Result<NewOrder, EngineError> {
    let order = parse_line(line)?; // ParseError -> EngineError::Parse
    validate(&order, balance)?; // OrderError -> EngineError::Order
    Ok(order)
}

// Print an error and every error that caused it.
fn print_chain(e: &dyn Error) {
    println!("  error: {e}");
    let mut cause = e.source();
    while let Some(c) = cause {
        println!("    caused by: {c}");
        cause = c.source();
    }
}

fn section_layered_errors() {
    println!("\n== layered errors ==");
    for line in [
        "BUY BTC-USD 2 100",
        "BUY BTC-USD 2 1o0",
        "HOLD BTC-USD 1 1",
        "SELL ETH-USD 0 100",
    ] {
        println!("  {line:?}");
        match submit(line, 10_000) {
            Ok(o) => println!("  ok: {o:?}"),
            Err(e) => print_chain(&e),
        }
    }

    // There's no From<ParseIntError> for EngineError, so `?` can't convert:
    // ❌ UNCOMMENT -> error[E0277]: `?` couldn't convert the error to `EngineError`
    // fn qty_only(s: &str) -> Result<u64, EngineError> { Ok(s.parse::<u64>()?) }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Box<dyn Error>: "any error" for scripts, main and prototypes
// ─────────────────────────────────────────────────────────────────────────────
//
// Box<dyn Error> holds ANY type that implements Error, and `?` converts into
// it automatically. It's close to `catch (e: unknown)`: quick to write, but
// callers can no longer match on the exact cases. (dyn is covered in phase
// 8, Box in phase 9.)

/// "BTC-USD=1,ETH-USD=5" -> [("BTC-USD", 1), ("ETH-USD", 5)]
fn load_tick_sizes(cfg: &str) -> Result<Vec<(String, i64)>, Box<dyn Error>> {
    let mut out = Vec::new();
    for entry in cfg.split(',') {
        // A &str converts into Box<dyn Error> too, handy for one-off messages.
        let (sym, tick) = entry.split_once('=').ok_or("expected SYMBOL=TICK")?;
        out.push((sym.to_string(), tick.parse::<i64>()?)); // ParseIntError -> Box<dyn Error>
    }
    Ok(out)
}

fn section_box_dyn_error() {
    println!("\n== Box<dyn Error> ==");
    println!("  {:?}", load_tick_sizes("BTC-USD=1,ETH-USD=5"));
    match load_tick_sizes("BTC-USD=1,ETH-USD") {
        Ok(_) => {}
        Err(e) => println!("  error: {e}"),
    }
    // If you need the concrete type back, you can ask for it ("downcast"):
    if let Err(e) = load_tick_sizes("BTC-USD=x") {
        let is_parse = e.downcast_ref::<ParseIntError>().is_some();
        println!("  error: {e} (is a ParseIntError: {is_parse})");
    }

    // main can return a Result as well:
    //     fn main() -> Result<(), Box<dyn Error>> { ...; Ok(()) }
    // If it returns Err, the program prints the error's Debug form and exits
    // with code 1.
    //
    // Two crates you'll meet in real projects (this lab sticks to std):
    //   thiserror  derives Display, Error and From for your error enums
    //   anyhow     a nicer Box<dyn Error> that lets you add context messages
    // Rule of thumb: use typed enums where callers react differently to each
    // case (the engine, the order API), and Box<dyn Error> or anyhow at the
    // top of a program, where you only log and exit.
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Validate once, at the boundary ("parse, don't validate")
// ─────────────────────────────────────────────────────────────────────────────
//
// Check data once, when it enters the system, and turn it into a type that
// CAN'T hold an invalid value. Code deeper inside never re-checks, and can't
// fail for that reason.

#[derive(Debug, Clone, Copy, PartialEq)]
struct Qty(u64); // invariant: always > 0

// TryFrom is the standard trait for conversions that can fail. Implementing
// it also gives you `.try_into()` for free.
impl TryFrom<u64> for Qty {
    type Error = OrderError;

    fn try_from(v: u64) -> Result<Self, Self::Error> {
        if v == 0 {
            Err(OrderError::ZeroQty)
        } else {
            Ok(Qty(v))
        }
    }
}

// The matching code takes a Qty, so it has no zero case to handle.
fn half(q: Qty) -> u64 {
    q.0.div_ceil(2)
}

fn section_validate_at_the_boundary() {
    println!("\n== validate at the boundary ==");
    let q = Qty::try_from(7);
    let zero: Result<Qty, _> = 0u64.try_into();
    println!("  try_from(7) = {q:?}, try_into(0) = {zero:?}");
    if let Ok(q) = q {
        println!("  half of {} = {}", q.0, half(q));
    }
    // One gap: in this single file, anything can still write Qty(0) directly.
    // Phase 12 makes the field private inside a module, so the only way to
    // build a Qty is through try_from.
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Errors inside iterators and closures
// ─────────────────────────────────────────────────────────────────────────────

fn section_errors_in_iterators() {
    println!("\n== errors in iterators ==");

    // All or nothing: collect into Result<Vec<_>, _> stops at the first error.
    let all: Result<Vec<u64>, _> = ["1", "2", "x", "4"].iter().map(|s| parse_qty(s)).collect();
    println!("  all-or-nothing: {all:?}");

    // Keep going: sort the results into accepted and rejected. For a batch
    // of orders, you usually want this. One bad order shouldn't block the rest.
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for s in ["1", "2", "x", "4"] {
        match parse_qty(s) {
            Ok(q) => accepted.push(q),
            Err(e) => rejected.push((s, e.to_string())),
        }
    }
    println!("  accepted {accepted:?}, rejected {rejected:?}");

    // `?` inside a closure returns from the CLOSURE, not from the outer
    // function. A closure that returns u64 can't use it:
    // ❌ UNCOMMENT -> error[E0277]: the `?` operator can only be used in a closure that returns `Result` or `Option`
    // let qtys: Vec<u64> = ["1", "2"].iter().map(|s| s.parse::<u64>()?).collect();

    // Fix: let the closure return the Result, and collect into Result<Vec<_>, _>
    // as above. Then use `?` on that, OUTSIDE the closure.
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. PUZZLES. Each block has a compile error. Uncomment one, read the error,
//    and fix it.
// ─────────────────────────────────────────────────────────────────────────────
fn puzzles() {
    println!("\n== puzzles (uncomment in source) ==");

    // PUZZLE 1: the error type is String, but parse fails with ParseIntError.
    // fn lots(s: &str) -> Result<u64, String> { let n: u64 = s.parse()?; Ok(n * 100) }
    // println!("{:?}", lots("3"));

    // PUZZLE 2: best bid, or an error if the book is empty.
    // fn best(bids: &[i64]) -> Result<i64, OrderError> { let b = bids.iter().max()?; Ok(*b) }
    // println!("{:?}", best(&[100, 101]));

    // PUZZLE 3: something is missing around the return value.
    // fn tick_for(symbol: &str) -> Result<i64, OrderError> { if symbol == "BTC-USD" { 1 } else { Err(OrderError::UnknownSymbol(symbol.to_string())) } }
    // println!("{:?}", tick_for("BTC-USD"));

    // PUZZLE 4: print a timeout error for a user.
    // #[derive(Debug)] struct Timeout { ms: u64 }
    // println!("{}", Timeout { ms: 250 });
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES: `cargo test --bin p07_errors`
// No unwrap() or expect() in your solutions. Use `?`, match and the combinators.
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Debug, PartialEq)]
#[allow(dead_code)]
enum PriceError {
    Empty,
    Negative,
    Invalid,
    TooManyDecimals { max: u32 },
}

/// Parse a decimal price into integer ticks with `decimals` decimal places.
/// With decimals = 2:
///   "650.25" -> 65025    "650" -> 65000    "650.2" -> 65020    " 1.5 " -> 150
///   "" or "   "         -> Err(Empty)
///   "-1.00"             -> Err(Negative)
///   "650.255"           -> Err(TooManyDecimals { max: 2 })
///   "abc", "1.2.3", "1." -> Err(Invalid)
/// Suggested order: trim; check Empty; check Negative; split_once('.');
/// parse both parts with .map_err(|_| PriceError::Invalid)? ; only then
/// compare the number of decimal digits with `decimals`.
/// (10_i64.pow(n) scales the whole part; the decimal part needs padding,
/// so "2" with 2 decimals means 20.)
#[allow(dead_code)]
fn parse_price(s: &str, decimals: u32) -> Result<i64, PriceError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(PriceError::Empty);
    }
    if s.starts_with('-') {
        return Err(PriceError::Negative);
    }

    // No '.' is fine ("650"); a '.' with nothing after it is not ("1.").
    let (whole_part, decimal_part) = match s.split_once('.') {
        Some((_, "")) => return Err(PriceError::Invalid),
        Some((w, d)) => (w, d),
        None => (s, ""),
    };

    // Only ASCII digits: rejects "abc", "1.2.3" (decimal part "2.3"), "1.-5", ".5".
    let all_digits = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
    if whole_part.is_empty() || !all_digits(whole_part) || !all_digits(decimal_part) {
        return Err(PriceError::Invalid);
    }

    // Count digits on the string, not the parsed number: "05" is 2 digits.
    let decimal_len = decimal_part.len() as u32;
    if decimal_len > decimals {
        return Err(PriceError::TooManyDecimals { max: decimals });
    }

    let whole: i64 = whole_part.parse().map_err(|_| PriceError::Invalid)?;
    let frac: i64 = if decimal_part.is_empty() {
        0
    } else {
        decimal_part.parse().map_err(|_| PriceError::Invalid)?
    };

    // "650.2" with 2 decimals: 650 * 100 + 2 * 10 (pad the fraction to 2 digits).
    // Checked maths: absurdly large input is Invalid, not a panic.
    whole
        .checked_mul(10_i64.pow(decimals))
        .and_then(|w| w.checked_add(frac * 10_i64.pow(decimals - decimal_len)))
        .ok_or(PriceError::Invalid)
}

/// Messages a client would see:
///   Empty                     -> "empty price"
///   Negative                  -> "negative price"
///   Invalid                   -> "invalid price"
///   TooManyDecimals { max: 2 } -> "too many decimals (max 2)"
impl fmt::Display for PriceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PriceError::Empty => write!(f, "empty price"),
            PriceError::Invalid => write!(f, "invalid price"),
            PriceError::Negative => write!(f, "negative price"),
            PriceError::TooManyDecimals { max } => {
                write!(f, "too many decimals (max {max})")
            }
        }
    }
}

#[derive(Debug, PartialEq)]
#[allow(dead_code)]
enum RiskError {
    PositionLimit { limit: i64, would_be: i64 },
}

/// Wrap a RiskError as EngineError::Risk, so `?` can convert it.
impl From<RiskError> for EngineError {
    fn from(e: RiskError) -> Self {
        EngineError::Risk(e)
    }
}

/// The position limit check. A buy adds qty to `position`, a sell subtracts
/// it. If the absolute value of the new position is above `limit`, return
/// Err(PositionLimit { limit, would_be: new_position }).
#[allow(dead_code)]
fn check_risk(o: &NewOrder, position: i64, limit: i64) -> Result<(), RiskError> {
    // Work in i64 (positions can be negative). qty as i64 only wraps above
    // i64::MAX, far beyond any real order.
    let delta = match o.side {
        Side::Buy => o.qty as i64,
        Side::Sell => -(o.qty as i64),
    };
    let would_be = position + delta;
    if would_be.abs() > limit {
        return Err(RiskError::PositionLimit { limit, would_be });
    }
    Ok(())
}

/// Parse, validate, then risk-check one line, in that order, returning the
/// first error as an EngineError. Use `?` for all three steps (you'll need
/// your From impl above).
#[allow(dead_code)]
fn submit_with_risk(
    line: &str,
    balance: i64,
    position: i64,
    limit: i64,
) -> Result<NewOrder, EngineError> {
    let order = parse_line(line)?; // ParseError -> EngineError::Parse
    validate(&order, balance)?; // OrderError -> EngineError::Order
    check_risk(&order, position, limit)?; // RiskError -> EngineError::Risk
    Ok(order)
}

/// Submit every line with `submit` (section 5) and the given balance. Return
/// (accepted orders, rejected (line index, error)), both in input order. One
/// bad line must not stop the others.
#[allow(dead_code)]
fn submit_batch(lines: &[&str], balance: i64) -> (Vec<NewOrder>, Vec<(usize, EngineError)>) {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        match submit(line, balance) {
            Ok(order) => accepted.push(order),
            Err(e) => rejected.push((i, e)),
        }
    }
    (accepted, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_parse_price() {
        assert_eq!(parse_price("650.25", 2), Ok(65_025));
        assert_eq!(parse_price("650", 2), Ok(65_000));
        assert_eq!(parse_price("650.2", 2), Ok(65_020));
        assert_eq!(parse_price(" 1.5 ", 2), Ok(150));
        assert_eq!(
            parse_price("1.5", 0),
            Err(PriceError::TooManyDecimals { max: 0 })
        );
        assert_eq!(parse_price("", 2), Err(PriceError::Empty));
        assert_eq!(parse_price("   ", 2), Err(PriceError::Empty));
        assert_eq!(parse_price("-1.00", 2), Err(PriceError::Negative));
        assert_eq!(
            parse_price("650.255", 2),
            Err(PriceError::TooManyDecimals { max: 2 })
        );
        assert_eq!(parse_price("abc", 2), Err(PriceError::Invalid));
        assert_eq!(parse_price("1.2.3", 2), Err(PriceError::Invalid));
        assert_eq!(parse_price("1.", 2), Err(PriceError::Invalid));
    }

    #[test]
    fn t_price_error_display() {
        assert_eq!(PriceError::Empty.to_string(), "empty price");
        assert_eq!(PriceError::Negative.to_string(), "negative price");
        assert_eq!(PriceError::Invalid.to_string(), "invalid price");
        assert_eq!(
            PriceError::TooManyDecimals { max: 2 }.to_string(),
            "too many decimals (max 2)"
        );
    }

    #[test]
    fn t_from_risk_error() {
        let e: EngineError = RiskError::PositionLimit {
            limit: 1,
            would_be: 2,
        }
        .into();
        assert_eq!(
            e,
            EngineError::Risk(RiskError::PositionLimit {
                limit: 1,
                would_be: 2
            })
        );
    }

    #[test]
    fn t_check_risk() {
        let buy = NewOrder {
            side: Side::Buy,
            symbol: "BTC-USD".into(),
            qty: 5,
            price: 100,
        };
        let sell = NewOrder {
            side: Side::Sell,
            ..buy.clone()
        };
        assert_eq!(check_risk(&buy, 0, 10), Ok(()));
        assert_eq!(
            check_risk(&buy, 6, 10),
            Err(RiskError::PositionLimit {
                limit: 10,
                would_be: 11
            })
        );
        assert_eq!(
            check_risk(&sell, -6, 10),
            Err(RiskError::PositionLimit {
                limit: 10,
                would_be: -11
            })
        );
        assert_eq!(check_risk(&sell, 6, 10), Ok(()));
    }

    #[test]
    fn t_submit_with_risk() {
        assert!(submit_with_risk("BUY BTC-USD 5 100", 10_000, 0, 10).is_ok());
        assert_eq!(
            submit_with_risk("BUY BTC-USD 5 100", 10_000, 8, 10),
            Err(EngineError::Risk(RiskError::PositionLimit {
                limit: 10,
                would_be: 13
            }))
        );
        // Parse and validation errors come first:
        assert_eq!(
            submit_with_risk("BUY BTC-USD", 10_000, 8, 10),
            Err(EngineError::Parse(ParseError::MissingField("qty")))
        );
        assert_eq!(
            submit_with_risk("BUY BTC-USD 5 101", 10_000, 8, 10),
            Err(EngineError::Order(OrderError::PriceOffTick {
                price: 101,
                tick: 5
            }))
        );
    }

    #[test]
    fn t_submit_batch() {
        let (ok, bad) = submit_batch(
            &[
                "BUY BTC-USD 5 100",
                "HOLD BTC-USD 1 1",
                "SELL ETH-USD 0 100",
                "SELL ETH-USD 1 100",
            ],
            10_000,
        );
        assert_eq!(ok.len(), 2);
        assert_eq!(ok[1].side, Side::Sell);
        assert_eq!(
            bad,
            vec![
                (1, EngineError::Parse(ParseError::BadSide("HOLD".into()))),
                (2, EngineError::Order(OrderError::ZeroQty)),
            ]
        );
    }
}
