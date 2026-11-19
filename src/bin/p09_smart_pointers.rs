//! PHASE 9 — Memory layout and smart pointers
//!
//!   cargo run  --bin p09_smart_pointers
//!   cargo test --bin p09_smart_pointers
//!
//! Phases 3–5 gave you two ways to hold data: OWN it (T) or BORROW it
//! (&T / &mut T). That covers most code. This phase covers the cases where
//! it doesn't:
//!
//!   Box<T>          one owner, but the value lives on the heap
//!   Rc<T>           SEVERAL owners (one thread). Freed when the last owner goes.
//!   Arc<T>          the same across threads
//!   Cell / RefCell  change data behind a shared reference ("interior mutability")
//!   Weak<T>         a non-owning pointer that breaks reference cycles
//!
//! First, though, what values actually look like in memory. For a trading
//! system that matters: how many bytes an order takes, and whether the data
//! you scan is laid out next to each other (fast) or scattered across the
//! heap behind pointers (slow).

use std::cell::{Cell, RefCell};
use std::mem::{align_of, offset_of, size_of};
use std::num::NonZeroU64;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::thread;

fn main() {
    section_layout();
    section_box();
    section_dsts_and_fat_pointers();
    section_rc();
    section_interior_mutability();
    section_weak_and_cycles();
    section_arc();
    section_choosing();
    puzzles();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Side {
    Buy,
    Sell,
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Memory layout: size, alignment, padding
// ─────────────────────────────────────────────────────────────────────────────
//
// Every type has a SIZE and an ALIGNMENT. A u64 must start at an address
// that's a multiple of 8, a u32 at a multiple of 4, and so on. To keep every
// field aligned, the compiler inserts unused PADDING bytes.
//
// With #[repr(C)], fields stay in the order you wrote them (C's rules, needed
// for talking to C code or for fixed binary formats):
//
//   #[repr(C)] struct MessyC { a: u8, b: u64, c: u8 }
//   offset: 0    1 ......... 7  8 ............. 15  16   17 ......... 23
//          [a ][ padding (7)  ][ b (8 bytes)       ][c ][ padding (7)  ]  = 24 bytes
//
// Without it (the default), Rust may REORDER fields to waste less space:
//          [ b (8 bytes)       ][a ][c ][ padding (6) ]                   = 16 bytes

#[allow(dead_code)]
struct Messy {
    a: u8,
    b: u64,
    c: u8,
}

#[allow(dead_code)]
#[repr(C)]
struct MessyC {
    a: u8,
    b: u64,
    c: u8,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Order {
    id: u64,
    price: i64,
    qty: u64,
    side: Side,
}

#[allow(dead_code)]
enum Event {
    Ack(u64),
    Fill { price: i64, qty: u64 },
    Reject(String),
}

fn section_layout() {
    println!("\n== layout ==");
    println!(
        "  Messy (Rust order): size {}, align {}",
        size_of::<Messy>(),
        align_of::<Messy>()
    );
    println!(
        "  MessyC (C order):   size {}, offsets a={} b={} c={}",
        size_of::<MessyC>(),
        offset_of!(MessyC, a),
        offset_of!(MessyC, b),
        offset_of!(MessyC, c)
    );
    println!(
        "  Order: size {} (3 × 8 bytes + 1 for side + padding)",
        size_of::<Order>()
    );

    // ENUMS: the size of the largest variant, plus space for the "tag" that
    // says which variant it is (unless the tag fits in unused bits).
    println!(
        "  Side: {}   Event: {}",
        size_of::<Side>(),
        size_of::<Event>()
    );

    // NICHE OPTIMISATION. Option<u64> needs an extra tag, because every u64
    // value is valid. But a reference or a Box can never be null, so Rust
    // uses null to mean None. Option costs nothing there.
    println!(
        "  u64 {} / Option<u64> {} / Option<NonZeroU64> {} / Option<&Order> {} / Option<Box<Order>> {}",
        size_of::<u64>(),
        size_of::<Option<u64>>(),
        size_of::<Option<NonZeroU64>>(),
        size_of::<Option<&Order>>(),
        size_of::<Option<Box<Order>>>()
    );
    // (An order id that's never 0 could be a NonZeroU64. Then Option<OrderId>
    // is 8 bytes instead of 16.)

    // WHY LAYOUT MATTERS. The CPU loads memory in 64-byte "cache lines". A
    // Vec<Order> stores orders back to back, so scanning it streams through
    // memory, and the hardware prefetches what comes next. A Vec<Box<Order>>
    // stores POINTERS, and each order sits wherever the allocator put it:
    // every step is a jump, and possibly a cache miss (~100 ns each).
    let inline: Vec<Order> = (0..3)
        .map(|i| Order {
            id: i,
            price: 100,
            qty: 1,
            side: Side::Buy,
        })
        .collect();

    // In a running program other allocations happen between your orders
    // (messages, strings, log lines). `noise` simulates that.
    let mut boxed: Vec<Box<Order>> = Vec::new();
    let mut noise: Vec<String> = Vec::new();
    for o in &inline {
        boxed.push(Box::new(o.clone()));
        noise.push("x".repeat(200 + 300 * o.id as usize));
    }
    let gap = |a: &Order, b: &Order| (b as *const Order as isize) - (a as *const Order as isize);
    println!(
        "  Vec<Order>:      orders 0→1 are {} bytes apart, 1→2 are {}",
        gap(&inline[0], &inline[1]),
        gap(&inline[1], &inline[2])
    );
    println!(
        "  Vec<Box<Order>>: orders 0→1 are {} bytes apart, 1→2 are {}",
        gap(&boxed[0], &boxed[1]),
        gap(&boxed[1], &boxed[2])
    );
    // The first line is always exactly size_of::<Order>(). The second depends
    // on what else was allocated, which is exactly the problem.
    drop(noise);
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Box<T>: one owner, on the heap
// ─────────────────────────────────────────────────────────────────────────────
//
// Box::new(v) moves v onto the heap. The Box itself is just a pointer
// (8 bytes) and it OWNS the value: when the Box is dropped, the value is
// dropped and the memory freed. You use it for:
//   1. recursive types (below)
//   2. trait objects: Box<dyn Strategy> (phase 8)
//   3. big values you want to move around cheaply (moving a Box copies 8 bytes)

// A recursive type, as an "expression tree" for pegged order prices:
// "best bid + 1, but never above mid". A PriceRule can contain PriceRules.
//
// Without Box this can't compile. How big would PriceRule be? Big enough to
// hold an Offset, which holds a PriceRule, which holds... forever.
// ❌ UNCOMMENT -> error[E0072]: recursive type `BadRule` has infinite size
// enum BadRule { Fixed(i64), Offset(BadRule, i64) }
//
// A Box is always 8 bytes, whatever it points to, so the size is known.

#[derive(Debug)]
#[allow(dead_code)]
enum PriceRule {
    Fixed(i64),
    BestBid,
    BestAsk,
    Mid,                                 // (bid + ask) / 2, rounded down
    Offset(Box<PriceRule>, i64),         // the inner rule's price + n
    Min(Box<PriceRule>, Box<PriceRule>), // the lower of two rules
}

fn section_box() {
    println!("\n== Box ==");

    let b = Box::new(Order {
        id: 1,
        price: 100,
        qty: 5,
        side: Side::Buy,
    });
    println!(
        "  size_of Box<Order> = {}, order lives at {:p}",
        size_of::<Box<Order>>(),
        b
    );
    println!(
        "  auto-deref: b.price = {}, explicit (*b).qty = {}",
        b.price,
        (*b).qty
    );
    let unboxed: Order = *b; // `*` moves the value back out of the box (and frees the box)
    println!("  moved out of the box: #{}", unboxed.id);

    // Build "best bid + 1, but never above mid":
    let peg = PriceRule::Min(
        Box::new(PriceRule::Offset(Box::new(PriceRule::BestBid), 1)),
        Box::new(PriceRule::Mid),
    );
    println!("  peg rule: {peg:?}");
    println!(
        "  size_of PriceRule = {} (fixed, thanks to Box)",
        size_of::<PriceRule>()
    );
    // Evaluating it is an exercise below.
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Dynamically sized types and fat pointers
// ─────────────────────────────────────────────────────────────────────────────
//
// Some types have no size known at compile time: str, [T] and dyn Trait.
// They're called DSTs (dynamically sized types). You can't hold one directly
// on the stack. You always reach them through a pointer, and that pointer
// carries the missing information alongside the address:
//
//   &[T], &str, Box<[T]>, Box<str>   address + LENGTH          = 16 bytes
//   &dyn Trait, Box<dyn Trait>       address + VTABLE pointer  = 16 bytes
//   &T, Box<T> for sized T           address only              =  8 bytes
//
// Generic functions assume T is sized unless you opt out with `?Sized`.

fn show_len<T: ?Sized + std::fmt::Debug>(x: &T) -> String {
    format!("{x:?}")
}

fn section_dsts_and_fat_pointers() {
    println!("\n== DSTs and fat pointers ==");
    println!(
        "  &u64 {}  &[u64] {}  &str {}  Box<str> {}  &dyn Debug {}",
        size_of::<&u64>(),
        size_of::<&[u64]>(),
        size_of::<&str>(),
        size_of::<Box<str>>(),
        size_of::<&dyn std::fmt::Debug>()
    );
    // Box<str> and Box<[T]> are a String/Vec that can't grow: 16 bytes instead
    // of 24 (no capacity field). Handy for data you build once and keep.
    let sym: Box<str> = "BTC-USD".into();
    println!(
        "  Box<str>: {sym}   show_len on a str: {}",
        show_len("works because of ?Sized")
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Rc<T>: shared ownership (one thread)
// ─────────────────────────────────────────────────────────────────────────────
//
// Sometimes a value has no single natural owner. Example: an Instrument's
// settings (symbol, tick size) are needed by every order on it, by the
// book, and by the risk engine. Who frees it? With Rc ("reference counted"),
// all of them OWN it together:
//
//   - Rc::new(v) puts v on the heap next to a counter, starting at 1.
//   - Rc::clone(&rc) does NOT copy v. It adds 1 to the counter and returns
//     another pointer to the same value.
//   - Dropping an Rc subtracts 1. When the count reaches 0, v is dropped.
//
// It's the GC-like behaviour you know from TS, except it's opt-in, you can
// see it, and freeing happens at a precise moment.

#[derive(Debug)]
struct Instrument {
    symbol: String,
    tick: i64,
}

impl Drop for Instrument {
    fn drop(&mut self) {
        println!("  (Instrument {} freed)", self.symbol);
    }
}

struct RestingOrder {
    id: u64,
    inst: Rc<Instrument>,
}

fn section_rc() {
    println!("\n== Rc ==");
    let btc = Rc::new(Instrument {
        symbol: "BTC-USD".into(),
        tick: 1,
    });
    println!("  count after new: {}", Rc::strong_count(&btc));

    let o1 = RestingOrder {
        id: 1,
        inst: Rc::clone(&btc),
    };
    let o2 = RestingOrder {
        id: 2,
        inst: Rc::clone(&btc),
    };
    println!("  count with two orders: {}", Rc::strong_count(&btc));
    println!("  same allocation? {}", Rc::ptr_eq(&o1.inst, &o2.inst));
    println!(
        "  order #{} trades {} (tick {})",
        o2.id, o2.inst.symbol, o2.inst.tick
    ); // auto-deref through the Rc

    // Write Rc::clone(&x), not x.clone(). Both do the same thing, but the
    // first tells readers "this just bumps a counter", not "deep copy".

    drop(o1);
    println!("  after dropping order #1: {}", Rc::strong_count(&btc));
    drop(btc);
    println!("  dropped the original handle; order #2 still keeps it alive");
    drop(o2); // the last owner: the Instrument is freed HERE
    println!("  done");

    // Rc only hands out SHARED access. If several owners could all write,
    // that would break the borrowing rule from phase 4:
    // ❌ UNCOMMENT -> error[E0594]: cannot assign to data in an `Rc`
    // let eth = Rc::new(Instrument { symbol: "ETH-USD".into(), tick: 5 });
    // eth.tick = 1;
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Interior mutability: Cell and RefCell
// ─────────────────────────────────────────────────────────────────────────────
//
// Normally you need &mut to change something. Cell and RefCell let you change
// data through a SHARED reference (&), by moving the safety check:
//
//   Cell<T>     For Copy values. You never get a reference to the inside,
//               only get() a copy or set() a new value, so there's nothing
//               to conflict. Zero cost.
//   RefCell<T>  For anything. borrow() and borrow_mut() hand out guards and
//               enforce the phase 4 rule (many readers OR one writer) at
//               RUNTIME. Breaking the rule is a PANIC, not a compile error.

struct FeedStats {
    msgs: Cell<u64>, // a counter updated from code that only has &FeedStats
}

fn on_message(stats: &FeedStats) {
    stats.msgs.set(stats.msgs.get() + 1);
}

#[derive(Debug)]
struct Account {
    cash: i64,
}

fn section_interior_mutability() {
    println!("\n== Cell and RefCell ==");

    let stats = FeedStats { msgs: Cell::new(0) };
    for _ in 0..3 {
        on_message(&stats);
    }
    println!("  messages counted through &: {}", stats.msgs.get());

    let acct = RefCell::new(Account { cash: 1_000 });
    {
        let r1 = acct.borrow(); // like &Account. Many at once are fine.
        let r2 = acct.borrow();
        println!("  two readers: {} {}", r1.cash, r2.cash);
        // A writer while readers exist: try_borrow_mut returns Err instead of panicking.
        println!(
            "  try_borrow_mut while reading: {:?}",
            acct.try_borrow_mut().map(|_| ())
        );
    } // r1 and r2 dropped here, so the reads are over
    acct.borrow_mut().cash -= 250; // like &mut Account, for this one statement
    println!("  after a write: {:?}", acct.borrow());

    // The same mistake as phase 4, but it compiles, and then crashes:
    // ❌ UNCOMMENT -> compiles, but PANICS: "RefCell already borrowed"
    // let reading = acct.borrow();
    // let writing = acct.borrow_mut();
    // println!("{} {}", reading.cash, writing.cash);

    // Rc<RefCell<T>>: several owners that can all change the value. Two
    // strategies sharing one account:
    let shared = Rc::new(RefCell::new(Account { cash: 1_000 }));
    let strat_a = Rc::clone(&shared);
    let strat_b = Rc::clone(&shared);
    strat_a.borrow_mut().cash -= 100;
    strat_b.borrow_mut().cash -= 300;
    println!(
        "  shared account after both strategies traded: {}",
        shared.borrow().cash
    );

    // Use this sparingly. Rc<RefCell<T>> is shared mutable state, the thing
    // Rust otherwise works hard to prevent: anyone holding a handle can change
    // it at any time, and a mistake is a runtime panic. In an engine, prefer
    // ONE owner that applies changes in order, with other parts sending it
    // messages (phase 10).
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Weak<T>, and the memory leak Rc can't prevent
// ─────────────────────────────────────────────────────────────────────────────
//
// If A holds an Rc to B and B holds an Rc to A, neither count can reach 0.
// Both stay alive forever: a LEAK. That's memory-safe (nothing dangles),
// but it's still a bug. TS's garbage collector finds cycles; Rc can't.
//
// The fix is Weak<T>: a pointer that does NOT count as an owner. To use it,
// you call upgrade(), which returns Option<Rc<T>>: Some if the value is
// still alive, None if it has been freed. Rule of thumb: parents own their
// children with Rc, children point back to parents with Weak.

struct LeakyDesk {
    name: String,
    orders: RefCell<Vec<Rc<LeakyOrder>>>,
}

struct LeakyOrder {
    desk: Rc<LeakyDesk>, // a STRONG back-pointer: this creates the cycle
}

impl Drop for LeakyDesk {
    fn drop(&mut self) {
        println!("  (LeakyDesk {} freed)", self.name); // never printed!
    }
}

struct Desk {
    name: String,
    orders: RefCell<Vec<Rc<DeskOrder>>>,
}

struct DeskOrder {
    id: u64,
    desk: Weak<Desk>, // a WEAK back-pointer: no cycle of owners
}

impl Drop for Desk {
    fn drop(&mut self) {
        println!("  (Desk {} freed)", self.name);
    }
}

fn section_weak_and_cycles() {
    println!("\n== Weak and cycles ==");
    {
        let desk = Rc::new(LeakyDesk {
            name: "leaky".into(),
            orders: RefCell::new(vec![]),
        });
        let o = Rc::new(LeakyOrder {
            desk: Rc::clone(&desk),
        });
        desk.orders.borrow_mut().push(Rc::clone(&o));
        println!(
            "  leaky: desk count {}, its order's desk is {}",
            Rc::strong_count(&desk),
            o.desk.name
        );
    } // desk and o go out of scope, but each is still owned by the other
    println!("  leaky desk scope ended: no 'freed' message above, so it leaked");

    {
        let desk = Rc::new(Desk {
            name: "rates".into(),
            orders: RefCell::new(vec![]),
        });
        let o = Rc::new(DeskOrder {
            id: 7,
            desk: Rc::downgrade(&desk),
        }); // Rc -> Weak
        desk.orders.borrow_mut().push(Rc::clone(&o));
        println!(
            "  weak: desk strong {} weak {}, desk has {} order(s)",
            Rc::strong_count(&desk),
            Rc::weak_count(&desk),
            desk.orders.borrow().len()
        );
        if let Some(d) = o.desk.upgrade() {
            println!("  order #{} belongs to desk {}", o.id, d.name);
        }
        drop(desk); // the only strong owner goes: Desk is freed now
        println!(
            "  after dropping the desk, upgrade() = {:?}",
            o.desk.upgrade().map(|d| d.name.clone())
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Arc<T>: Rc for threads
// ─────────────────────────────────────────────────────────────────────────────
//
// Rc's counter is a plain integer. If two threads cloned the same Rc at the
// same moment, both could read 5 and both write 6: a lost update (the race
// from phase 4), and later a use-after-free. So Rc is not allowed to cross
// threads at all. Arc ("atomically reference counted") updates its counter
// with atomic CPU instructions instead. It's safe across threads and costs a
// little more per clone and drop. Phase 10 builds on this.

fn section_arc() {
    println!("\n== Arc ==");
    let inst = Arc::new(Instrument {
        symbol: "SOL-USD".into(),
        tick: 1,
    });

    let mut handles = Vec::new();
    for worker in 0..3 {
        let inst = Arc::clone(&inst); // each thread gets its own handle
        handles.push(thread::spawn(move || {
            format!("worker {worker} sees {} (tick {})", inst.symbol, inst.tick)
        }));
    }
    for h in handles {
        println!("  {}", h.join().unwrap());
    }
    println!("  count back to {}", Arc::strong_count(&inst));

    // The compiler stops Rc from crossing threads:
    // ❌ UNCOMMENT -> error[E0277]: `Rc<Instrument>` cannot be sent between threads safely
    // let local = Rc::new(Instrument { symbol: "X".into(), tick: 1 });
    // thread::spawn(move || println!("{}", local.symbol)).join().unwrap();

    // Arc, like Rc, only gives SHARED access. To change shared data from
    // several threads you need Arc<Mutex<T>> or Arc<RwLock<T>> (phase 10).
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Choosing, and the alternative: indexes into a Vec
// ─────────────────────────────────────────────────────────────────────────────
//
//   T                you own it; it lives inline (on the stack or inside its parent)
//   &T / &mut T      temporary access, checked at compile time, free
//   Box<T>           one owner, on the heap: recursive types, dyn Trait, big values
//   Rc<T>            several owners, one thread, read-only
//   Rc<RefCell<T>>   several owners, one thread, mutable (rules checked at runtime)
//   Arc<T>           several owners, many threads, read-only
//   Arc<Mutex<T>>    several owners, many threads, mutable (phase 10)
//
// Go down the list only when the line above doesn't fit.
//
// Many Rust order books don't use Rc at all. They keep every order in one
// Vec and refer to orders by INDEX (a usize) everywhere else: from the price
// levels, the id map, the client's order list. That's called an "arena" or a
// "slab":
//   - no reference counting, no lifetimes, no RefCell
//   - orders sit next to each other in memory (section 1)
//   - cancelling is O(1): set the slot to None and remember it for reuse
// The trade-off: an index isn't checked like a reference. If you keep an old
// index after its slot was reused, you'll look at the wrong order. (Real
// slabs add a "generation" number to catch that.) You'll build one below.

fn section_choosing() {
    println!("\n== choosing ==");
    println!("  see the table in the source, then build the Slab exercise");
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. PUZZLES. Each block has a compile error. Uncomment one, read the error,
//    and fix it.
// ─────────────────────────────────────────────────────────────────────────────
fn puzzles() {
    println!("\n== puzzles (uncomment in source) ==");

    // PUZZLE 1: a linked list of price levels.
    // struct LevelNode { price: i64, next: Option<LevelNode> }
    // let l = LevelNode { price: 100, next: None };
    // println!("{}", l.price);

    // PUZZLE 2: count fills from inside a closure that only gets `&`.
    // let fills = 0;
    // let record = |_qty: u64| fills += 1;
    // fn run(f: impl Fn(u64)) { f(1); f(2); }
    // run(record);
    // println!("{fills}");

    // PUZZLE 3: share an account with a background thread.
    // let acct = Rc::new(RefCell::new(Account { cash: 10 }));
    // let a2 = Rc::clone(&acct);
    // thread::spawn(move || a2.borrow_mut().cash += 1).join().unwrap();

    // PUZZLE 4: add a fill to a shared order. Why can't Rc do this alone?
    // let order = Rc::new(Order { id: 1, price: 100, qty: 5, side: Side::Buy });
    // order.qty -= 1;
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES: `cargo test --bin p09_smart_pointers`
// ═════════════════════════════════════════════════════════════════════════════

// PREDICT: replace each 0 with the size in bytes, WITHOUT running anything
// first. Then run the tests and work out why you got any wrong.
#[allow(dead_code)]
#[repr(C)]
struct Packet {
    a: u8,
    b: u32,
    c: u8,
}

#[allow(dead_code)]
const PREDICT_OPTION_U32: usize = 8; // size_of::<Option<u32>>()
#[allow(dead_code)]
const PREDICT_OPTION_BOX: usize = 8; // size_of::<Option<Box<u64>>>()
#[allow(dead_code)]
const PREDICT_SLICE_REF: usize = 16; // size_of::<&[u64]>()
#[allow(dead_code)]
const PREDICT_BOX_DYN: usize = 16; // size_of::<Box<dyn Fn()>>()
#[allow(dead_code)]
const PREDICT_RC: usize = 8; // size_of::<Rc<String>>()
#[allow(dead_code)]
const PREDICT_PACKET: usize = 12; // size_of::<Packet>()  (note the repr(C))

impl PriceRule {
    /// Work out the price this rule gives for the current top of book.
    /// Mid is (bid + ask) / 2, rounded down. Offset adds n to the inner
    /// rule's price. Min is the lower of the two. Hint: recursion. Call
    /// .eval on the inner rules; `inner` is a &Box<PriceRule>, and method
    /// calls auto-deref through the Box.
    #[allow(dead_code)]
    fn eval(&self, bid: i64, ask: i64) -> i64 {
        let mid = (bid + ask) / 2;
        match self {
            PriceRule::Fixed(p) => *p,
            PriceRule::Mid => mid,
            PriceRule::Offset(inner, n) => inner.eval(bid, ask) + n,
            PriceRule::Min(a, b) => a.eval(bid, ask).min(b.eval(bid, ask)),
            PriceRule::BestBid => bid,
            PriceRule::BestAsk => ask,
        }
    }
}

/// An arena of values, addressed by index. Freed slots are REUSED: insert
/// takes the most recently freed slot if there is one, otherwise it appends.
#[allow(dead_code)]
struct Slab<T> {
    slots: Vec<Option<T>>,
    free: Vec<usize>, // indexes of empty slots
    len: usize,       // how many slots are occupied
}

#[allow(dead_code)]
impl<T> Slab<T> {
    fn new() -> Self {
        Slab {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
        }
    }

    /// Store `v`, return its index.
    fn insert(&mut self, v: T) -> usize {
        self.len += 1;
        let index = self.free.pop();
        match index {
            Some(index) => {
                self.slots[index] = Some(v);
                index
            }
            None => {
                self.slots.push(Some(v));
                self.slots.len() - 1
            }
        }
    }

    /// Take the value out. None if `key` is out of range or already empty.
    fn remove(&mut self, key: usize) -> Option<T> {
        if key >= self.len() {
            return None;
        }

        let value = self.slots[key].take();
        match value {
            Some(_) => {
                self.free.push(key);
                self.len -= 1;
            }
            None => {}
        }
        value
    }

    fn get(&self, key: usize) -> Option<&T> {
        self.slots[key].as_ref()
    }

    fn get_mut(&mut self, key: usize) -> Option<&mut T> {
        self.slots[key].as_mut()
    }

    fn len(&self) -> usize {
        self.len
    }
}

/// Create an order that points back to `desk` WITHOUT keeping it alive.
#[allow(dead_code)]
fn attach(desk: &Rc<Desk>, id: u64) -> DeskOrder {
    DeskOrder {
        id,
        desk: Rc::downgrade(desk),
    }
}

impl DeskOrder {
    /// The desk's name, or None if the desk has been freed.
    #[allow(dead_code)]
    fn desk_name(&self) -> Option<String> {
        let desk = &self.desk;
        // desk.upgrade()

        if let Some(d) = desk.upgrade() {
            Some(d.name.clone())
        } else {
            None
        }
    }
}

/// Move `amount` of cash between two accounts that may be shared.
///   - If `from` and `to` are the SAME RefCell, return Err("same account").
///     (Why must you check that BEFORE borrowing? Try it without the check.)
///   - If `from` has less than `amount`, return Err("insufficient funds").
///
/// Hint: std::ptr::eq(from, to) compares two references by address.
#[allow(dead_code)]
fn transfer(from: &RefCell<Account>, to: &RefCell<Account>, amount: i64) -> Result<(), String> {
    if from.as_ptr() == to.as_ptr() {
        return Err("same account".to_string());
    }

    if from.borrow().cash < amount {
        return Err("insufficient funds".to_string());
    }

    to.borrow_mut().cash += amount;
    from.borrow_mut().cash -= amount;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_predictions() {
        assert_eq!(PREDICT_OPTION_U32, size_of::<Option<u32>>(), "Option<u32>");
        assert_eq!(
            PREDICT_OPTION_BOX,
            size_of::<Option<Box<u64>>>(),
            "Option<Box<u64>>"
        );
        assert_eq!(PREDICT_SLICE_REF, size_of::<&[u64]>(), "&[u64]");
        assert_eq!(PREDICT_BOX_DYN, size_of::<Box<dyn Fn()>>(), "Box<dyn Fn()>");
        assert_eq!(PREDICT_RC, size_of::<Rc<String>>(), "Rc<String>");
        assert_eq!(PREDICT_PACKET, size_of::<Packet>(), "Packet");
    }

    #[test]
    fn t_price_rule() {
        use PriceRule::*;
        assert_eq!(Fixed(42).eval(100, 110), 42);
        assert_eq!(Mid.eval(100, 105), 102);
        let join = Offset(Box::new(BestBid), 1);
        assert_eq!(join.eval(100, 110), 101);
        let peg = Min(
            Box::new(Offset(Box::new(BestBid), 1)),
            Box::new(Offset(Box::new(BestAsk), -1)),
        );
        assert_eq!(peg.eval(100, 110), 101);
        assert_eq!(peg.eval(100, 101), 100); // a one-tick spread: never cross the ask
    }

    #[test]
    fn t_slab() {
        let mut s = Slab::new();
        let a = s.insert("a");
        let b = s.insert("b");
        let c = s.insert("c");
        assert_eq!((a, b, c), (0, 1, 2));
        assert_eq!(s.remove(b), Some("b"));
        assert_eq!(s.remove(b), None); // already empty
        assert_eq!(s.remove(99), None); // out of range
        assert_eq!(s.len(), 2);
        assert_eq!(s.get(b), None);
        assert_eq!(s.insert("d"), 1); // reuses the freed slot
        if let Some(v) = s.get_mut(0) {
            *v = "A";
        }
        assert_eq!(s.get(0), Some(&"A"));
        assert_eq!(s.len(), 3);
        assert_eq!(s.insert("e"), 3); // no free slots left, so it appends
    }

    #[test]
    fn t_weak_desk() {
        let desk = Rc::new(Desk {
            name: "fx".into(),
            orders: RefCell::new(vec![]),
        });
        let o = attach(&desk, 1);
        assert_eq!(o.id, 1);
        assert_eq!(
            Rc::strong_count(&desk),
            1,
            "the order must not own the desk"
        );
        assert_eq!(o.desk_name(), Some("fx".to_string()));
        drop(desk);
        assert_eq!(o.desk_name(), None);
    }

    #[test]
    fn t_transfer() {
        let a = Rc::new(RefCell::new(Account { cash: 100 }));
        let b = Rc::new(RefCell::new(Account { cash: 0 }));
        let a_again = Rc::clone(&a);
        assert_eq!(transfer(&a, &b, 60), Ok(()));
        assert_eq!((a.borrow().cash, b.borrow().cash), (40, 60));
        assert_eq!(transfer(&a, &b, 41), Err("insufficient funds".to_string()));
        assert_eq!(transfer(&a, &a_again, 1), Err("same account".to_string()));
        assert_eq!(a.borrow().cash, 40);
    }
}
