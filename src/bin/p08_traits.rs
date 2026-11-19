//! PHASE 8 — Traits and generics in depth
//!
//!   cargo run  --bin p08_traits
//!   cargo test --bin p08_traits
//!
//! Phase 2 showed the basics: define a trait, implement it, use it as a
//! bound. This phase covers how abstraction really works in Rust, and what
//! the compiler turns it into.
//!
//!   TS                                   Rust
//!   interface Venue { place(...) }       trait Venue { fn place(&mut self, ...); }
//!   class Paper implements Venue         impl Venue for Paper { ... }
//!   function f(v: Venue)                 fn f<V: Venue>(v: &mut V)    static dispatch
//!                                        fn f(v: &mut dyn Venue)      dynamic dispatch
//!   operator overloading                 impl Add for Qty
//!
//! The big difference: in TS every interface call is looked up at runtime.
//! Rust lets you CHOOSE. Generics compile to direct calls, a separate copy
//! for each type, with zero overhead. `dyn Trait` looks the method up at
//! runtime, like TS, when you need different types mixed together.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

fn main() {
    section_defining_traits();
    section_generics_and_bounds();
    section_operator_traits();
    section_ord_and_priority();
    section_associated_types();
    section_static_vs_dynamic();
    section_impl_trait();
    section_coherence_and_blanket_impls();
    puzzles();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Buy,
    Sell,
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Defining and implementing traits
// ─────────────────────────────────────────────────────────────────────────────

/// Something you can send orders to: a real exchange, a simulator, a test stub.
trait Venue {
    // REQUIRED methods: every implementor has to write these.
    fn name(&self) -> &str;
    fn place(&mut self, side: Side, price: i64, qty: u64) -> Result<u64, String>;

    // PROVIDED (default) method: built from the required ones. Implementors
    // get it for free, and may override it.
    fn place_both(&mut self, bid: i64, ask: i64, qty: u64) -> Result<(u64, u64), String> {
        let b = self.place(Side::Buy, bid, qty)?;
        let a = self.place(Side::Sell, ask, qty)?;
        Ok((b, a))
    }
}

/// A simulated venue that accepts everything and hands out ids.
struct PaperVenue {
    next_id: u64,
}

impl Venue for PaperVenue {
    fn name(&self) -> &str {
        "paper"
    }
    fn place(&mut self, _side: Side, _price: i64, _qty: u64) -> Result<u64, String> {
        self.next_id += 1;
        Ok(self.next_id)
    }
}

/// A venue that's down. Handy in tests.
struct ClosedVenue;

impl Venue for ClosedVenue {
    fn name(&self) -> &str {
        "closed"
    }
    fn place(&mut self, _: Side, _: i64, _: u64) -> Result<u64, String> {
        Err("venue is closed".to_string())
    }
}

// Traits are NOMINAL, unlike TS's structural interfaces. A type implements a
// trait only when someone writes `impl Trait for Type`; having methods with
// the right names isn't enough. In exchange, you can add YOUR trait to types
// you didn't write, even i64. This is an "extension trait":
trait TicksExt {
    fn fmt_price(self, decimals: u32) -> String;
}

impl TicksExt for i64 {
    fn fmt_price(self, decimals: u32) -> String {
        let scale = 10_i64.pow(decimals);
        format!(
            "{}.{:0width$}",
            self / scale,
            self % scale,
            width = decimals as usize
        )
    }
}

fn section_defining_traits() {
    println!("\n== defining traits ==");
    let mut paper = PaperVenue { next_id: 0 };
    // Call place_both BEFORE the println. name() returns a &str borrowed from
    // `paper`, so it can't overlap with place_both's &mut borrow (phase 4).
    let ids = paper.place_both(99, 101, 5);
    println!("  {}: place_both -> {ids:?}", paper.name());
    println!(
        "  {}: place_both -> {:?}",
        ClosedVenue.name(),
        ClosedVenue.place_both(99, 101, 5)
    );
    println!("  6500025.fmt_price(2) = {}", 6_500_025_i64.fmt_price(2));

    // A trait's methods are only callable where the trait is IN SCOPE. If
    // TicksExt lived in another module, you'd need `use that_module::TicksExt;`
    // first, or you'd get error[E0599]: no method named `fmt_price` found.
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Generics, bounds and `where`
// ─────────────────────────────────────────────────────────────────────────────

// Bound inline:
fn route_all<V: Venue>(venue: &mut V, orders: &[(Side, i64, u64)]) -> Vec<Result<u64, String>> {
    orders
        .iter()
        .map(|&(s, p, q)| venue.place(s, p, q))
        .collect()
}

// The same thing with `where`, which reads better once there are several
// type parameters or long bounds:
fn describe_pair<A, B>(a: &A, b: &B) -> String
where
    A: Venue,
    B: fmt::Debug + Clone,
{
    format!("{} with {:?}", a.name(), b.clone())
}

// A generic STRUCT. The bound goes on the impl block, not the struct, so a
// Spread<T> can exist for any T, but `.width()` only exists when T supports
// subtraction and copying. (Writing `T: Sub<Output = T>` is how you say
// "T - T gives a T". Output is an associated type, section 5.)
#[derive(Debug)]
struct Spread<T> {
    bid: T,
    ask: T,
}

impl<T: Sub<Output = T> + Copy> Spread<T> {
    fn width(&self) -> T {
        self.ask - self.bid
    }
}

// An impl just for one concrete type adds methods only Spread<i64> has:
impl Spread<i64> {
    fn mid_x2(&self) -> i64 {
        self.bid + self.ask
    }
}

fn section_generics_and_bounds() {
    println!("\n== generics and bounds ==");
    let mut paper = PaperVenue { next_id: 100 };
    println!(
        "  route_all: {:?}",
        route_all(&mut paper, &[(Side::Buy, 99, 1), (Side::Sell, 101, 1)])
    );
    println!("  {}", describe_pair(&paper, &Side::Buy));

    let s = Spread { bid: 99, ask: 101 };
    println!("  width={} mid_x2={}", s.width(), s.mid_x2());
    let f = Spread {
        bid: 0.99,
        ask: 1.01,
    };
    println!("  float width={:.2}", f.width()); // f64 has Sub + Copy, so width() exists

    // Side can't be subtracted, so a Spread<Side> exists, but has no width():
    let odd = Spread {
        bid: Side::Buy,
        ask: Side::Sell,
    };
    println!("  {odd:?}");
    // ❌ UNCOMMENT -> error[E0599]: the method `width` exists for struct `Spread<Side>`, but its trait bounds were not satisfied
    // println!("{:?}", odd.width());
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Operator traits: make newtypes pleasant to use
// ─────────────────────────────────────────────────────────────────────────────
//
// `a + b` is just a call to `Add::add(a, b)`. Implement Add for your type and
// `+` works. This makes the newtype pattern from phase 2 practical: you keep
// the type safety without writing `.0` everywhere.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
struct Qty(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Price(i64);

impl Add for Qty {
    type Output = Qty; // what `Qty + Qty` produces
    fn add(self, rhs: Qty) -> Qty {
        Qty(self.0 + rhs.0)
    }
}

impl AddAssign for Qty {
    fn add_assign(&mut self, rhs: Qty) {
        self.0 += rhs.0;
    }
}

// The types can encode UNITS: price × qty is a notional, not a price.
impl Mul<Qty> for Price {
    type Output = i64;
    fn mul(self, rhs: Qty) -> i64 {
        self.0 * rhs.0 as i64
    }
}

// Sum lets `.sum()` produce a Qty from an iterator of Qty.
impl Sum for Qty {
    fn sum<I: Iterator<Item = Qty>>(iter: I) -> Qty {
        iter.fold(Qty(0), |a, b| a + b)
    }
}

// From<u64> gives you `Qty::from(5)` AND `let q: Qty = 5.into()`. Implement
// From; Into comes for free (a blanket impl, section 8).
impl From<u64> for Qty {
    fn from(v: u64) -> Self {
        Qty(v)
    }
}

impl fmt::Display for Qty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} lots", self.0)
    }
}

fn section_operator_traits() {
    println!("\n== operator traits ==");
    let mut filled = Qty(3) + Qty(4);
    filled += Qty(1);
    let total: Qty = [Qty(1), Qty(2), Qty(3)].into_iter().sum();
    let from: Qty = 7.into();
    println!(
        "  filled={filled} total={total} into={from} default={:?}",
        Qty::default()
    );
    println!("  notional = {}", Price(101) * Qty(5));

    // Only the combinations you implement exist. Qty × Qty means nothing, so:
    // ❌ UNCOMMENT -> error[E0369]: cannot multiply `Qty` by `Qty`
    // let nonsense = Qty(2) * Qty(3);
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Ord by hand: price-time priority
// ─────────────────────────────────────────────────────────────────────────────
//
// #[derive(PartialOrd, Ord)] compares fields top to bottom. Sometimes you
// need a different order, so you write it yourself. Ord requires Eq and
// PartialOrd as well, and they must agree with it.
//
// BinaryHeap is a MAX-heap: pop() always returns the GREATEST element. So if
// "greater" means "better priority", the heap pops orders in matching order.

/// An ask in the queue. Better = LOWER price, then EARLIER arrival (seq).
#[derive(Debug, PartialEq, Eq)]
struct AskKey {
    price: i64,
    seq: u64,
}

impl Ord for AskKey {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed on purpose: `other` vs `self`, so a lower price compares
        // as GREATER. `.then(...)` breaks ties using the next rule.
        other.price.cmp(&self.price).then(other.seq.cmp(&self.seq))
    }
}

// Always write PartialOrd in terms of Ord like this, so they can't disagree.
impl PartialOrd for AskKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn section_ord_and_priority() {
    println!("\n== Ord and priority ==");
    let mut asks = BinaryHeap::new();
    asks.push(AskKey { price: 102, seq: 1 });
    asks.push(AskKey { price: 101, seq: 3 });
    asks.push(AskKey { price: 101, seq: 2 });
    asks.push(AskKey { price: 103, seq: 0 });
    print!("  match order:");
    while let Some(k) = asks.pop() {
        print!(" {}@seq{}", k.price, k.seq);
    }
    println!();
    // (A BTreeMap of price levels, as in phase 6, is usually better for a real
    // book, because it also supports cancel and walking levels. A heap is
    // great for "always give me the best one", like timers or event queues.)
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Associated types and constants
// ─────────────────────────────────────────────────────────────────────────────
//
// Iterator is the most famous trait with an ASSOCIATED TYPE:
//
//     trait Iterator {
//         type Item;                                   // chosen by each implementor
//         fn next(&mut self) -> Option<Self::Item>;    // the ONE required method
// ...plus ~75 provided methods: map, filter, sum, take, zip...
//     }
//
// Implement `next` and you get every adapter from phase 6 for free.

/// Hands out order ids forever: 1, 2, 3, ...
struct IdGen {
    next: u64,
}

impl Iterator for IdGen {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        self.next += 1;
        Some(self.next) // never None, so this iterator is infinite
    }
}

// Your own trait with an associated type. Each feed produces ONE kind of
// message, decided by the implementor.
trait Feed {
    type Msg: fmt::Debug; // a bound on the associated type
    const NAME: &'static str; // an associated CONSTANT

    fn poll(&mut self) -> Option<Self::Msg>;
}

#[derive(Debug)]
#[allow(dead_code)]
struct Trade {
    price: i64,
    qty: u64,
}

struct TradeFeed {
    pending: Vec<Trade>,
}

impl Feed for TradeFeed {
    type Msg = Trade;
    const NAME: &'static str = "trades";
    fn poll(&mut self) -> Option<Trade> {
        self.pending.pop()
    }
}

struct HeartbeatFeed {
    left: u32,
}

impl Feed for HeartbeatFeed {
    type Msg = u32;
    const NAME: &'static str = "heartbeats";
    fn poll(&mut self) -> Option<u32> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        Some(self.left)
    }
}

// Generic code refers to the message type as F::Msg.
fn drain<F: Feed>(feed: &mut F) -> Vec<F::Msg> {
    let mut out = Vec::new();
    while let Some(m) = feed.poll() {
        out.push(m);
    }
    out
}

fn section_associated_types() {
    println!("\n== associated types ==");
    let ids: Vec<u64> = IdGen { next: 0 }.take(3).collect();
    let even: Vec<u64> = IdGen { next: 0 }.filter(|i| i % 2 == 0).take(3).collect();
    println!("  ids {ids:?}, even ids {even:?}");

    let mut trades = TradeFeed {
        pending: vec![Trade { price: 100, qty: 2 }, Trade { price: 101, qty: 1 }],
    };
    println!("  {}: {:?}", TradeFeed::NAME, drain(&mut trades));
    println!(
        "  {}: {:?}",
        HeartbeatFeed::NAME,
        drain(&mut HeartbeatFeed { left: 3 })
    );

    // Associated type or generic parameter?
    //   trait Feed { type Msg; }   each type implements Feed ONCE, with one Msg.
    //   trait Feed<M> { ... }      a type could implement Feed<Trade> AND
    //                              Feed<Quote>, and every call would have to
    //                              say which one it means.
    // If each implementor has one natural answer, use an associated type.
    // That's why Iterator::Item, Add::Output and Deref::Target are all
    // associated types.
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Static vs dynamic dispatch: what the compiler actually does
// ─────────────────────────────────────────────────────────────────────────────

trait Strategy {
    fn name(&self) -> String;
    /// Look at a quote, maybe return an order (side, price).
    fn on_quote(&mut self, bid: i64, ask: i64) -> Option<(Side, i64)>;
}

/// Buys when the spread is wide, at one tick above the bid.
struct WideSpread {
    min_spread: i64,
}

impl Strategy for WideSpread {
    fn name(&self) -> String {
        format!("wide-spread({})", self.min_spread)
    }
    fn on_quote(&mut self, bid: i64, ask: i64) -> Option<(Side, i64)> {
        (ask - bid >= self.min_spread).then_some((Side::Buy, bid + 1))
    }
}

/// Sells after the bid has gone up `n` times in a row.
struct Momentum {
    n: u32,
    ups: u32,
    last_bid: i64,
}

impl Strategy for Momentum {
    fn name(&self) -> String {
        format!("momentum({})", self.n)
    }
    fn on_quote(&mut self, bid: i64, _ask: i64) -> Option<(Side, i64)> {
        self.ups = if bid > self.last_bid { self.ups + 1 } else { 0 };
        self.last_bid = bid;
        (self.ups >= self.n).then_some((Side::Sell, bid))
    }
}

// STATIC dispatch. The compiler makes a separate copy of this function for
// every S it's called with ("monomorphization"). Each copy calls on_quote
// directly, so it can be inlined and optimised as if you'd written it by hand.
fn run_static<S: Strategy>(s: &mut S, quotes: &[(i64, i64)]) -> usize {
    println!("    run_static compiled for {}", std::any::type_name::<S>());
    quotes.iter().filter_map(|&(b, a)| s.on_quote(b, a)).count()
}

// DYNAMIC dispatch. ONE copy of this function exists. `&mut dyn Strategy` is
// a "fat pointer": a pointer to the data, plus a pointer to a VTABLE, a table
// of function pointers for that type's methods. Each call looks the method
// up in that table at runtime.
//
//     &mut dyn Strategy            vtable for Momentum (one per type)
//     ┌──────────┬──────────┐      ┌────────────────────────────────┐
//     │ data ptr │ vtbl ptr ├────► │ drop, size, align              │
//     └────┬─────┴──────────┘      │ name     -> Momentum::name     │
//          ▼                       │ on_quote -> Momentum::on_quote │
//     Momentum { n, ups, .. }      └────────────────────────────────┘
fn run_dynamic(s: &mut dyn Strategy, quotes: &[(i64, i64)]) -> usize {
    quotes.iter().filter_map(|&(b, a)| s.on_quote(b, a)).count()
}

fn section_static_vs_dynamic() {
    println!("\n== static vs dynamic dispatch ==");
    let quotes = [(100, 104), (101, 102), (102, 105), (103, 104)];

    let mut w = WideSpread { min_spread: 3 };
    let mut m = Momentum {
        n: 2,
        ups: 0,
        last_bid: 0,
    };
    println!(
        "  static:  {} signals, {} signals",
        run_static(&mut w, &quotes),
        run_static(&mut m, &quotes)
    );

    // What dyn buys you: DIFFERENT types in ONE collection, chosen at runtime
    // (from a config file, say). With generics, a Vec<S> holds one type only.
    let mut strategies: Vec<Box<dyn Strategy>> = vec![
        Box::new(WideSpread { min_spread: 2 }),
        Box::new(Momentum {
            n: 1,
            ups: 0,
            last_bid: 0,
        }),
    ];
    for s in strategies.iter_mut() {
        let n = run_dynamic(s.as_mut(), &quotes);
        println!("  dynamic: {} -> {n} signals", s.name());
    }

    println!(
        "  size_of &WideSpread = {}, &dyn Strategy = {} (data + vtable pointers)",
        std::mem::size_of::<&WideSpread>(),
        std::mem::size_of::<&dyn Strategy>()
    );

    // Choosing between them:
    //   generics   fastest (inlined), but a bigger binary, and types are fixed
    //              at compile time. Use them on the HOT PATH: matching, risk.
    //   dyn Trait  one indirect call per method (a few ns) and no inlining,
    //              but lets you mix types at runtime. Use it at the EDGES:
    //              plugins, a list of strategies, venue connectors.
    //   enum       a third option when you know all the types up front:
    //              enum AnyStrategy { Wide(WideSpread), Mom(Momentum) } plus a
    //              match. No heap, no vtable, and the compiler checks you
    //              handled every case. Very common in trading code.

    // Not every trait can be used with `dyn`. A generic METHOD can't go in a
    // vtable, because there'd be one copy per type it's called with:
    // ❌ UNCOMMENT -> error[E0038]: the trait `Exporter` is not dyn compatible
    // trait Exporter { fn export<W: std::io::Write>(&self, out: W); }
    // fn save(e: &dyn Exporter) { let _ = e; }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. `impl Trait` in arguments and return types
// ─────────────────────────────────────────────────────────────────────────────

// In ARGUMENT position, `impl Trait` is short for a generic parameter:
// `fn f(s: &mut impl Strategy)` means `fn f<S: Strategy>(s: &mut S)`.
fn first_signal(s: &mut impl Strategy, quotes: &[(i64, i64)]) -> Option<(Side, i64)> {
    quotes.iter().find_map(|&(b, a)| s.on_quote(b, a))
}

// In RETURN position it means "one specific type that I'm not naming". You
// saw it in phase 6 for returning closures and iterators, whose types you
// can't write out.
fn ticks_between(lo: i64, hi: i64, tick: i64) -> impl Iterator<Item = i64> {
    (lo..=hi).step_by(tick as usize)
}

// But it must be ONE type. Picking between two needs Box<dyn Strategy>:
fn make_strategy(aggressive: bool) -> Box<dyn Strategy> {
    if aggressive {
        Box::new(Momentum {
            n: 1,
            ups: 0,
            last_bid: 0,
        })
    } else {
        Box::new(WideSpread { min_spread: 5 })
    }
}

fn section_impl_trait() {
    println!("\n== impl Trait ==");
    let mut w = WideSpread { min_spread: 3 };
    println!(
        "  first signal: {:?}",
        first_signal(&mut w, &[(100, 101), (100, 104)])
    );
    println!(
        "  ticks: {:?}",
        ticks_between(100, 120, 5).collect::<Vec<_>>()
    );
    println!("  made: {}", make_strategy(true).name());

    // ❌ UNCOMMENT -> error[E0308]: `if` and `else` have incompatible types
    // fn make_bad(aggressive: bool) -> impl Strategy { if aggressive { Momentum { n: 1, ups: 0, last_bid: 0 } } else { WideSpread { min_spread: 5 } } }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Supertraits, blanket impls, and the orphan rule
// ─────────────────────────────────────────────────────────────────────────────

// SUPERTRAIT: anything that's Auditable must also be Debug, so default
// methods can use `{:?}` on self.
trait Auditable: fmt::Debug {
    fn audit_line(&self) -> String {
        format!("[audit] {self:?}")
    }
}

impl Auditable for Qty {}
impl Auditable for Side {}

// BLANKET IMPL: implement a trait for EVERY type that meets a bound. The
// standard library does this a lot:
//     impl<T: Display> ToString for T      (why everything with Display has .to_string())
//     impl<T, U: From<T>> Into<U> for T    (why implementing From gives you Into)
trait Shout {
    fn shout(&self) -> String;
}

impl<T: fmt::Display> Shout for T {
    fn shout(&self) -> String {
        self.to_string().to_uppercase()
    }
}

fn section_coherence_and_blanket_impls() {
    println!("\n== supertraits, blanket impls, orphan rule ==");
    println!("  {}", Qty(5).audit_line());
    println!("  {}", Side::Sell.audit_line());
    println!(
        "  {} / {} / {}",
        "filled".shout(),
        Qty(3).shout(),
        42.shout()
    );

    // THE ORPHAN RULE: you can implement a trait for a type only if the trait
    // OR the type is defined in your crate. Otherwise two libraries could
    // both implement Display for Vec<Side> and the compiler couldn't pick one.
    // ❌ UNCOMMENT -> error[E0117]: only traits defined in the current crate can be implemented for types defined outside of the crate
    // impl fmt::Display for Vec<i64> { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.len()) } }

    // Fix: wrap it in your own type (the newtype pattern again).
    struct Ladder(Vec<i64>);
    impl fmt::Display for Ladder {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(
                f,
                "{} levels from {:?} to {:?}",
                self.0.len(),
                self.0.first(),
                self.0.last()
            )
        }
    }
    println!("  {}", Ladder(vec![100, 101, 102]));
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. PUZZLES. Each block has a compile error. Uncomment one, read the error,
//    and fix it.
// ─────────────────────────────────────────────────────────────────────────────
fn puzzles() {
    println!("\n== puzzles (uncomment in source) ==");

    // PUZZLE 1: return the larger of two values of any type.
    // fn larger<T>(a: T, b: T) -> T { if a > b { a } else { b } }
    // println!("{}", larger(3, 9));

    // PUZZLE 2: print any venue's name. (Two possible fixes; know both.)
    // fn show(v: Venue) { println!("{}", v.name()); }
    // show(PaperVenue { next_id: 0 });

    // PUZZLE 3: a list of strategies.
    // let all: Vec<dyn Strategy> = vec![];
    // println!("{}", all.len());

    // PUZZLE 4: implement a trait, but forget part of it.
    // struct Silent;
    // impl Strategy for Silent { fn name(&self) -> String { "silent".into() } }
    // println!("{}", Silent.name());
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES: `cargo test --bin p08_traits`
// ═════════════════════════════════════════════════════════════════════════════

/// Qty - Qty. Taking away more than you have is a bug in the engine, so
/// panic with the message "qty underflow". Hint: u64::checked_sub + expect.
impl Sub for Qty {
    type Output = Qty;
    fn sub(self, rhs: Qty) -> Qty {
        if rhs.0 > self.0 {
            panic!("qty underflow");
        }

        Qty(self.0 - rhs.0)
    }
}

/// `q -= other`. Reuse your Sub impl.
impl SubAssign for Qty {
    fn sub_assign(&mut self, rhs: Qty) {
        *self = *self - rhs;
    }
}

/// A bid in the queue. Better = HIGHER price, then EARLIER arrival (lower seq).
/// Make BinaryHeap pop bids in matching order (see AskKey in section 4).
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
struct BidKey {
    price: i64,
    seq: u64,
}

impl Ord for BidKey {
    fn cmp(&self, other: &Self) -> Ordering {
        other.price.cmp(&self.price).then(other.seq.cmp(&self.seq))
    }
}

impl PartialOrd for BidKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Yields prices from `from` up to and including `to`, `tick` apart. If `to`
/// isn't on a tick, stop at the last price below it. No ranges or step_by:
/// keep the state in the struct.
#[allow(dead_code)]
struct TickLadder {
    next: i64,
    to: i64,
    tick: i64,
}

impl Iterator for TickLadder {
    type Item = i64;
    fn next(&mut self) -> Option<i64> {
        if self.next > self.to {
            return None;
        }

        let cur = self.next;
        self.next += self.tick;
        Some(cur)
    }
}

/// Build a strategy from a config name:
///   "wide:N"     -> WideSpread { min_spread: N }
///   "momentum:N" -> Momentum { n: N, ups: 0, last_bid: 0 }
///   anything else, or a bad number -> None
#[allow(dead_code)]
fn strategy_from_config(s: &str) -> Option<Box<dyn Strategy>> {
    let Some((raw_strategy, raw_input)) = s.split_once(':') else {
        return None;
    };

    if raw_strategy == "wide" {
        let Some(parsed_n) = raw_input.parse::<i64>().ok() else {
            return None;
        };

        return Some(Box::new(WideSpread {
            min_spread: parsed_n,
        }));
    }

    if raw_strategy == "momentum" {
        let Some(parsed_n) = raw_input.parse::<u32>().ok() else {
            return None;
        };

        return Some(Box::new(Momentum {
            n: parsed_n,
            ups: 0,
            last_bid: 0,
        }));
    }

    return None;
}

/// Return the item with the LARGEST key (the first one on ties), or None if
/// `items` is empty. Generic over the item type T and the key type K.
/// Careful: Iterator::max_by_key returns the LAST maximum (phase 6).
#[allow(dead_code)]
fn best_by<T, K: Ord>(items: &[T], key: impl Fn(&T) -> K) -> Option<&T> {
    if items.is_empty() {
        return None;
    }

    let mut largest_item = &items[0];

    for item in items {
        if key(item) > key(largest_item) {
            largest_item = item;
        }
    }

    Some(largest_item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_qty_sub() {
        assert_eq!(Qty(5) - Qty(3), Qty(2));
        let mut q = Qty(10);
        q -= Qty(4);
        assert_eq!(q, Qty(6));
    }

    #[test]
    #[should_panic(expected = "qty underflow")]
    fn t_qty_sub_underflow() {
        let _ = Qty(1) - Qty(2);
    }

    #[test]
    fn t_bid_priority() {
        let mut heap = BinaryHeap::new();
        for (price, seq) in [(100, 1), (101, 4), (101, 2), (99, 0)] {
            heap.push(BidKey { price, seq });
        }
        let order: Vec<(i64, u64)> = std::iter::from_fn(|| heap.pop())
            .map(|k| (k.price, k.seq))
            .collect();
        assert_eq!(order, vec![(101, 2), (101, 4), (100, 1), (99, 0)]);
    }

    #[test]
    fn t_tick_ladder() {
        let v: Vec<i64> = TickLadder {
            next: 100,
            to: 110,
            tick: 5,
        }
        .collect();
        assert_eq!(v, vec![100, 105, 110]);
        let v: Vec<i64> = TickLadder {
            next: 100,
            to: 112,
            tick: 5,
        }
        .collect();
        assert_eq!(v, vec![100, 105, 110]);
        assert_eq!(
            TickLadder {
                next: 100,
                to: 99,
                tick: 1
            }
            .count(),
            0
        );
        // You get every adapter for free:
        let s: i64 = TickLadder {
            next: 1,
            to: 4,
            tick: 1,
        }
        .map(|p| p * 10)
        .sum();
        assert_eq!(s, 100);
    }

    #[test]
    fn t_strategy_from_config() {
        assert_eq!(
            strategy_from_config("wide:3").map(|s| s.name()),
            Some("wide-spread(3)".to_string())
        );
        assert_eq!(
            strategy_from_config("momentum:2").map(|s| s.name()),
            Some("momentum(2)".to_string())
        );
        assert!(strategy_from_config("wide:x").is_none());
        assert!(strategy_from_config("arb:1").is_none());
        assert!(strategy_from_config("wide").is_none());
    }

    #[test]
    fn t_best_by() {
        let v = [3, 9, 2, 9];
        let best = best_by(&v, |x| *x).unwrap();
        assert!(std::ptr::eq(best, &v[1])); // the FIRST 9
        let words = ["bid", "offer", "ask"];
        assert_eq!(best_by(&words, |w| w.len()), Some(&"offer"));
        let empty: [i64; 0] = [];
        assert_eq!(best_by(&empty, |x| *x), None);
    }
}
