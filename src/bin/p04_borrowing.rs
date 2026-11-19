//! PHASE 4 — Borrowing
//!
//!   cargo run  --bin p04_borrowing
//!   cargo test --bin p04_borrowing
//!
//! Phase 3 ended with a clumsy pattern: pass an Order in, get it back out.
//! A BORROW (a reference) lets code use a value without taking ownership.
//!
//!   &T      shared reference: read-only, any number at the same time
//!   &mut T  mutable reference: read-write, EXCLUSIVE (only one at a time)
//!
//! THE BORROWING RULE. At any moment a value has EITHER
//!   - any number of `&T`, OR
//!   - exactly one `&mut T`,
//!
//! and no reference may outlive the value's owner.
//!
//! In TS every object reference behaves like a `&mut` that anyone can hold:
//! any function you pass an object to can change it, at any time, even while
//! a loop somewhere else is reading it. Rust makes "shared" and "mutable"
//! mutually exclusive, and checks it at compile time. That rule is also what
//! makes threads safe later on (phase 10).

use std::mem;

fn main() {
    section_shared_refs();
    section_mut_refs();
    section_the_rule();
    section_borrows_end_at_last_use();
    section_reborrowing();
    section_slices();
    section_borrowing_in_loops_and_match();
    section_methods_and_self();
    section_disjoint_borrows();
    puzzles();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq)]
struct Order {
    id: u64,
    side: Side,
    price: i64,
    qty: u64,
    client: String,
}

fn order(id: u64, side: Side, price: i64, qty: u64, client: &str) -> Order {
    Order {
        id,
        side,
        price,
        qty,
        client: client.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Shared references: &T
// ─────────────────────────────────────────────────────────────────────────────

// Borrows the order. The caller keeps ownership; we only look.
fn describe(o: &Order) -> String {
    // `o.price` works directly: `.` dereferences automatically, so you never
    // write `(*o).price`.
    format!(
        "#{} {:?} {}@{} for {}",
        o.id, o.side, o.qty, o.price, o.client
    )
}

fn notional(o: &Order) -> i64 {
    o.price * o.qty as i64
}

fn section_shared_refs() {
    println!("\n== shared references (&T) ==");

    let o = order(1, Side::Buy, 100, 5, "alice");
    println!("  {}", describe(&o)); // `&o` creates the borrow
    println!("  notional = {}", notional(&o));
    println!("  still own it: #{}", o.id); // nothing was moved

    // A reference is just a pointer: 8 bytes, whatever it points at.
    let r: &Order = &o;
    println!("  size_of &Order = {}", mem::size_of::<&Order>());
    println!("  r points at {:p}, o lives at {:p}", r, &o); // same address

    // Any number of shared borrows can exist at once. &T is Copy, so copying
    // a reference copies the pointer, not the Order.
    let r1 = &o;
    let r2 = &o;
    let r3 = r1;
    println!("  r1={} r2={} r3={}", r1.id, r2.id, r3.id);

    // Read-only means read-only:
    // ❌ UNCOMMENT -> error[E0594]: cannot assign to `r.qty`, which is behind a `&` reference
    // r.qty = 4;

    // For plain numbers you sometimes need `*` to get from the reference to
    // the value. `.` does it for you on fields and methods; operators like
    // `==` don't.
    let px = 100;
    let px_ref = &px;
    println!("  *px_ref == 100 ? {}", *px_ref == 100);
    // ❌ UNCOMMENT -> error[E0277]: can't compare `&{integer}` with `{integer}`
    // println!("{}", px_ref == 100);
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Mutable references: &mut T
// ─────────────────────────────────────────────────────────────────────────────

// Can change the caller's order in place. No move, no return value needed.
fn reduce(o: &mut Order, by: u64) {
    o.qty -= by.min(o.qty);
}

fn bump(counter: &mut u64) {
    *counter += 1; // writing through a reference to a plain value needs `*`
}

fn section_mut_refs() {
    println!("\n== mutable references (&mut T) ==");

    let mut o = order(1, Side::Buy, 100, 5, "alice");
    reduce(&mut o, 2); // the caller writes `&mut` too, so mutation is visible at the call
    println!("  after reduce: qty={}", o.qty);

    let mut fills = 0;
    bump(&mut fills);
    bump(&mut fills);
    println!("  fills = {fills}");

    // You can only lend out `&mut` of something you're allowed to mutate:
    // ❌ UNCOMMENT -> error[E0596]: cannot borrow `frozen` as mutable, as it is not declared as mutable
    // let frozen = order(2, Side::Sell, 101, 1, "bob");
    // reduce(&mut frozen, 1);

    // The signature tells the reader what a function can do with your value:
    //   fn f(o: Order)       takes it (you lose it)
    //   fn f(o: &Order)      looks at it
    //   fn f(o: &mut Order)  may change it
    // In TS, `function f(o: Order)` could do any of these, and you'd have to
    // read the body to find out.
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. The rule: shared XOR mutable
// ─────────────────────────────────────────────────────────────────────────────

fn section_the_rule() {
    println!("\n== shared XOR mutable ==");

    let mut o = order(1, Side::Buy, 100, 5, "alice");

    let a = &o;
    let b = &o;
    println!("  two readers: {} {}", a.qty, b.qty);

    let m = &mut o; // fine: `a` and `b` are never used again
    m.qty += 1;
    println!("  one writer: qty={}", m.qty);

    // ❌ UNCOMMENT -> error[E0499]: cannot borrow `o` as mutable more than once at a time
    // let m1 = &mut o;
    // let m2 = &mut o;
    // m1.qty += 1;
    // m2.qty += 1;

    // ❌ UNCOMMENT -> error[E0502]: cannot borrow `o` as mutable because it is also borrowed as immutable
    // let reader = &o;
    // let writer = &mut o;
    // writer.qty = 0;
    // println!("{}", reader.qty);

    // WHY the rule exists, part 1: it's the same bug as a race condition.
    // You've probably hit this one in Node, with no threads involved:
    //
    //   const bal = account.balance;      // read
    //   await riskCheck(order);           // other handlers run here...
    //   account.balance = bal - cost;     // ...and this write loses theirs
    //
    // A race needs (1) two parties that can reach the same data, (2) at least
    // one of them writing, and (3) their accesses overlapping in time. Rust's
    // rule removes (1)+(2): while anyone can write (`&mut`), nobody else can
    // even read. Phase 10 uses the same rule to make threads safe.
    //
    // WHY the rule exists, part 2. Watch a Vec move its elements when it grows:
    let mut book: Vec<Order> = Vec::with_capacity(1);
    book.push(order(1, Side::Buy, 100, 5, "alice"));
    let before = book.as_ptr();
    book.push(order(2, Side::Buy, 99, 5, "bob")); // no room: allocate a bigger buffer, move everything
    println!(
        "  elements were at {:p}, now at {:p}",
        before,
        book.as_ptr()
    );

    // So a reference into the old buffer would point at freed memory. In C++
    // that's "iterator invalidation", undefined behaviour. In Rust it doesn't
    // compile:
    // ❌ UNCOMMENT -> error[E0502]: cannot borrow `book` as mutable because it is also borrowed as immutable
    // let first = &book[0];
    // book.push(order(3, Side::Buy, 98, 5, "carol"));
    // println!("{}", first.id);

    // Fix: copy out what you need first (u64 is Copy), then mutate.
    let first_id = book[0].id;
    book.push(order(3, Side::Buy, 98, 5, "carol"));
    println!("  first id was {first_id}, book now has {}", book.len());
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Borrows end at their LAST USE, not at the end of the scope
// ─────────────────────────────────────────────────────────────────────────────

fn section_borrows_end_at_last_use() {
    println!("\n== borrows end at last use ==");

    let mut o = order(1, Side::Buy, 100, 5, "alice");
    let r = &o;
    println!("  read through r: {}", r.qty); // last use of `r`, so the borrow ends here
    o.qty += 1; // fine, even though `r` is technically still in scope
    println!("  mutated: {}", o.qty);

    // The compiler looks at where each reference is USED ("non-lexical
    // lifetimes", NLL). Many borrow errors are fixed just by moving a line.

    // A borrow passed to a function lasts only for the call, unless the
    // function returns a reference (section 8).
    let n = notional(&o); // n is a plain i64; the borrow is already over
    o.qty = 0;
    println!("  notional before reset = {n}, qty now {}", o.qty);
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Reborrowing, and `&` vs `*`
// ─────────────────────────────────────────────────────────────────────────────
//
//   &x   makes a reference to x
//   *r   follows the reference r to the value it points at
//
// `&mut T` is NOT Copy, because two copies would mean two writers. So how can
// you pass the same `&mut` to several calls in a row? REBORROWING. When you
// pass `m` where a `&mut T` is expected, the compiler actually passes
// `&mut *m`: a new, shorter borrow made THROUGH m. While the reborrow is in
// use, `m` is frozen. When it ends, `m` works again.

fn section_reborrowing() {
    println!("\n== reborrowing ==");

    let mut o = order(1, Side::Buy, 100, 10, "alice");
    let m: &mut Order = &mut o;

    reduce(m, 1); // really reduce(&mut *m, 1): m is lent out for this call only
    reduce(m, 1); // so m is usable again afterwards
    println!("  qty after two reduces: {}", m.qty);

    // Assigning to a new variable with no type annotation MOVES the &mut instead:
    // ❌ UNCOMMENT -> error[E0382]: borrow of moved value: `m`
    // let m2 = m;
    // m2.qty += 1;
    // println!("{}", m.qty);

    // An explicit reborrow keeps m usable once m2 is done:
    let m2 = &mut *m;
    m2.qty += 1;
    println!("  via explicit reborrow: {}", m.qty); // m2's last use was above, so m is free

    // While a reborrow is still in use, the original is frozen:
    // ❌ UNCOMMENT -> error[E0503]: cannot use `m.qty` because it was mutably borrowed
    let m3 = &mut *m;
    // m.qty += 1;
    m3.qty += 1;

    // A `&mut` can also be reborrowed as SHARED (`&*m`), which happens
    // automatically when you pass it to a function that only reads:
    println!("  notional through &mut: {}", notional(m));

    // You need `*` for plain values behind a reference (`*counter += 1`), and
    // to replace the whole value a reference points at:
    *m = order(2, Side::Sell, 101, 1, "bob"); // overwrites o; the old Order is dropped
    println!("  replaced through m: o is now #{}", o.id);
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Slices: &str and &[T], borrowed views
// ─────────────────────────────────────────────────────────────────────────────
//
//   &str  borrowed view into UTF-8 text:              (ptr, len) = 16 bytes
//   &[T]  borrowed view into a run of T's in memory:   (ptr, len) = 16 bytes
//
//         String / Vec<T>                    heap
//         ┌─────┬─────┬─────┐               ┌───┬───┬───┬───┬───┬───┬───┐
//         │ ptr │ len │ cap │ ────────────► │ B │ T │ C │ - │ U │ S │ D │
//         └─────┴─────┴─────┘               └───┴───┴───┴───┴───┴───┴───┘
//                                             ▲
//         &symbol[0..3]  ┌─────┬─────┐        │
//                        │ ptr │len=3│ ───────┘   no copy, just a window
//                        └─────┴─────┘
//
// For PARAMETERS, prefer `&str` over `&String` and `&[T]` over `&Vec<T>`.
// They accept more callers (literals, sub-ranges, arrays) and cost nothing:
// `&String` turns into `&str` automatically ("deref coercion").

fn count_buys(orders: &[Order]) -> usize {
    let mut n = 0;
    for o in orders {
        if o.side == Side::Buy {
            n += 1;
        }
    }
    n
}

fn is_usd_pair(symbol: &str) -> bool {
    symbol.ends_with("-USD")
}

fn section_slices() {
    println!("\n== slices ==");

    let symbol = String::from("BTC-USD");
    println!("  {} {}", is_usd_pair(&symbol), is_usd_pair("ETH-EUR")); // &String and a literal both work

    let base: &str = &symbol[0..3]; // byte range 0..3
    println!(
        "  base={base} shares memory with symbol: {}",
        base.as_ptr() == symbol.as_ptr()
    );
    println!(
        "  size_of &str = {}, String = {}",
        mem::size_of::<&str>(),
        mem::size_of::<String>()
    );

    let book = vec![
        order(1, Side::Buy, 100, 5, "alice"),
        order(2, Side::Sell, 101, 2, "bob"),
        order(3, Side::Buy, 99, 1, "carol"),
        order(4, Side::Buy, 98, 3, "dave"),
    ];
    println!("  buys in whole book: {}", count_buys(&book)); // &Vec<Order> -> &[Order]
    println!("  buys in book[1..3]: {}", count_buys(&book[1..3])); // a sub-range, no copying
    println!(
        "  first={:?} last={:?}",
        book.first().map(|o| o.id),
        book.last().map(|o| o.id)
    );
    let (top, rest) = book.split_at(2);
    println!("  split_at(2): {} + {}", top.len(), rest.len());
    println!("  book.get(2..10) = {:?}", book.get(2..10).map(|s| s.len())); // None instead of a panic

    // String slicing uses BYTE offsets, and both ends must fall on a char
    // boundary. Non-ASCII characters take several bytes ("€" is 3).
    // ❌ UNCOMMENT -> compiles, but PANICS: "end byte index 1 is not a char boundary; it is inside '€'"
    // let price_label = "€100";
    // println!("{}", &price_label[0..1]);
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Borrowing in loops and pattern matching
// ─────────────────────────────────────────────────────────────────────────────

fn section_borrowing_in_loops_and_match() {
    println!("\n== loops & matches ==");

    let mut book = vec![
        order(1, Side::Buy, 100, 5, "alice"),
        order(2, Side::Buy, 99, 2, "bob"),
    ];

    // Three ways to loop over a Vec:
    //   for o in &book      o: &Order       read          (book.iter())
    //   for o in &mut book  o: &mut Order   modify        (book.iter_mut())
    //   for o in book       o: Order        consume it    (phase 3)
    print!(" ");
    for o in &book {
        print!(" #{}", o.id);
    }
    println!();
    for o in &mut book {
        o.qty *= 2; // `.` derefs through the &mut
    }
    println!(
        "  doubled: {:?}",
        book.iter().map(|o| o.qty).collect::<Vec<_>>()
    );

    // You can't change a Vec while you're looping over it:
    // ❌ UNCOMMENT -> error[E0502]: cannot borrow `book` as mutable because it is also borrowed as immutable
    // for o in &book {
    //     book.push(order(o.id + 100, Side::Sell, o.price + 1, o.qty, "mm"));
    // }

    // Fix: collect the changes first, apply them after the loop.
    let mut quotes = Vec::new();
    for o in &book {
        quotes.push(order(o.id + 100, Side::Sell, o.price + 1, o.qty, "mm"));
    }
    book.extend(quotes); // extend consumes `quotes` and pushes each item
    println!("  book now has {} orders", book.len());

    // Matching on a REFERENCE binds the inner values by reference
    // automatically. This is the fix previewed in phase 3:
    let maybe_reason: Option<String> = Some(String::from("cancel-reason"));
    if let Some(reason) = &maybe_reason {
        // reason: &String
        println!("  reason={reason}");
    }
    println!("  still have it: {maybe_reason:?}");

    // Many std methods hand out references wrapped in Option:
    match book.first() {
        // Option<&Order>
        Some(o) => println!("  first is #{}", o.id),
        None => println!("  empty"),
    }

    if let Some(o) = book.get_mut(1) {
        // Option<&mut Order>
        o.qty = 1;
    }
    if let Some(last) = book.last_mut() {
        last.price += 1;
    }
    println!(
        "  book[1].qty={} last.price={}",
        book[1].qty,
        book[book.len() - 1].price
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Methods: &self, &mut self, self
// ─────────────────────────────────────────────────────────────────────────────

struct Book {
    bids: Vec<Order>,
    asks: Vec<Order>,
}

impl Book {
    fn new() -> Self {
        Book {
            bids: Vec::new(),
            asks: Vec::new(),
        }
    }

    fn add(&mut self, o: Order) {
        match o.side {
            Side::Buy => self.bids.push(o),
            Side::Sell => self.asks.push(o),
        }
    }

    // Returns a reference INTO the book. The caller can look at the order
    // without copying it, but the book stays borrowed for as long as the
    // returned reference is used. (Phase 5 explains how the compiler knows
    // the result borrows from `self`.)
    fn best_bid(&self) -> Option<&Order> {
        let mut best: Option<&Order> = None;
        for o in &self.bids {
            match best {
                Some(b) if b.price >= o.price => {} // keep the earlier one on ties
                _ => best = Some(o),
            }
        }
        best
    }

    fn bids_mut(&mut self) -> &mut Vec<Order> {
        &mut self.bids
    }

    fn asks_mut(&mut self) -> &mut Vec<Order> {
        &mut self.asks
    }

    // Takes `self`, so it consumes the book.
    fn into_orders(self) -> Vec<Order> {
        let mut all = self.bids;
        all.extend(self.asks);
        all
    }
}

fn section_methods_and_self() {
    println!("\n== methods ==");

    let mut book = Book::new();
    // `book.add(..)` really means `Book::add(&mut book, ..)`. The `&mut` is
    // added for you ("auto-ref"), so method calls look the same as in TS.
    book.add(order(1, Side::Buy, 100, 5, "alice"));
    book.add(order(2, Side::Buy, 101, 1, "bob"));
    book.add(order(3, Side::Sell, 103, 2, "carol"));

    if let Some(b) = book.best_bid() {
        println!("  best bid #{} @ {}", b.id, b.price);
    }

    // THE situation you'll hit most in a matching engine: you look at the
    // best order, then want to change the book.
    // ❌ UNCOMMENT -> error[E0502]: cannot borrow `book` as mutable because it is also borrowed as immutable
    // let best = book.best_bid().unwrap();
    // book.add(order(4, Side::Buy, best.price + 1, 1, "eve"));
    // println!("improved on #{}", best.id);

    // Fix: copy the small Copy fields you need out of the reference. The
    // borrow ends there, and you're free to mutate.
    let best_px = book.best_bid().unwrap().price;
    book.add(order(4, Side::Buy, best_px + 1, 1, "eve"));
    println!("  new best bid #{}", book.best_bid().unwrap().id);

    let all = book.into_orders(); // book is gone now
    println!("  consumed book into {} orders", all.len());
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Disjoint borrows: two &mut at once, when they can't overlap
// ─────────────────────────────────────────────────────────────────────────────

fn section_disjoint_borrows() {
    println!("\n== disjoint borrows ==");

    let mut book = Book::new();
    book.add(order(1, Side::Buy, 100, 5, "alice"));
    book.add(order(2, Side::Sell, 100, 3, "bob"));

    // Two DIFFERENT fields can be borrowed mutably at the same time. Inside
    // one function the compiler tracks each field separately.
    let bids = &mut book.bids;
    let asks = &mut book.asks;
    if let (Some(b), Some(a)) = (bids.first_mut(), asks.first_mut()) {
        let traded = b.qty.min(a.qty);
        b.qty -= traded;
        a.qty -= traded;
        println!("  traded {traded}: bid left {}, ask left {}", b.qty, a.qty);
    }

    // A METHOD call borrows ALL of `self`. The compiler goes by the
    // signature, not the body, so it can't see that these touch different
    // fields:
    // ❌ UNCOMMENT -> error[E0499]: cannot borrow `book` as mutable more than once at a time
    // let b = book.bids_mut();
    // let a = book.asks_mut();
    // b.clear();
    // a.clear();

    // Fixes: borrow the fields directly as above, or write one method that
    // returns both: fn sides_mut(&mut self) -> (&mut Vec<Order>, &mut Vec<Order>)
    book.bids_mut().retain(|o| o.qty > 0);
    book.asks_mut().retain(|o| o.qty > 0);
    println!(
        "  after cleanup: {} bids, {} asks",
        book.bids.len(),
        book.asks.len()
    );

    // The same problem with two elements of one array or Vec. Indexes are
    // usually runtime values, so the compiler can't prove they're different:
    let mut v = [
        order(1, Side::Buy, 100, 5, "alice"),
        order(2, Side::Sell, 100, 9, "bob"),
    ];
    // ❌ UNCOMMENT -> error[E0499]: cannot borrow `v[_]` as mutable more than once at a time
    // let x = &mut v[0];
    // let y = &mut v[1];
    // x.qty -= 1;
    // y.qty -= 1;

    // split_at_mut gives you two non-overlapping &mut slices, which proves it.
    let (left, right) = v.split_at_mut(1); // left = v[..1], right = v[1..]
    let (x, y) = (&mut left[0], &mut right[0]);
    mem::swap(&mut x.qty, &mut y.qty);
    println!("  swapped qtys: {} {}", v[0].qty, v[1].qty);
    v.swap(0, 1); // swapping whole elements has its own method
    println!("  swapped orders: #{} #{}", v[0].id, v[1].id);
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. PUZZLES. Each block has a compile error. Uncomment one, read the error,
//     and fix it properly (no .clone()!).
// ─────────────────────────────────────────────────────────────────────────────
fn puzzles() {
    println!("\n== puzzles (uncomment in source) ==");

    // PUZZLE 1: show an order twice. Make `show` borrow (the calls change too).
    // fn show(o: Order) { println!("#{}", o.id); }
    // let o = order(1, Side::Buy, 100, 1, "alice");
    // show(o);
    // show(o);

    // PUZZLE 2: halve every order's qty. (There are two things to fix.)
    // let book = vec![order(1, Side::Buy, 100, 10, "a"), order(2, Side::Buy, 99, 6, "b")];
    // for o in &book { o.qty /= 2; }

    // PUZZLE 3: remember the best bid's price, then clear the bids.
    // let mut bids = vec![order(1, Side::Buy, 100, 1, "a")];
    // let best = &bids[0];
    // bids.clear();
    // println!("best was {}", best.price);

    // PUZZLE 4: build a label and return it. Why can't a lifetime fix this?
    // fn label(o: &Order) -> &str { let s = format!("#{}@{}", o.id, o.price); &s }
    // println!("{}", label(&order(1, Side::Buy, 100, 1, "a")));
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES: `cargo test --bin p04_borrowing`
// Rule: no .clone() and no new Strings. Borrow everything.
// ═════════════════════════════════════════════════════════════════════════════

/// Total qty over all orders.
#[allow(dead_code)]
fn total_qty(orders: &[Order]) -> u64 {
    let mut total = 0;
    for o in orders {
        total += o.qty;
    }
    total
}

/// The lowest-priced order (the earliest one if prices tie), or None if the
/// slice is empty. Return a reference INTO the slice; the test checks that
/// it really points at the original.
#[allow(dead_code)]
fn best_ask(asks: &[Order]) -> Option<&Order> {
    if asks.is_empty() {
        return None;
    }

    let mut lowest_price_index = 0;

    for (i, ask) in asks.iter().enumerate() {
        if ask.price < asks[lowest_price_index].price {
            lowest_price_index = i;
        }
    }

    asks.get(lowest_price_index)
}

/// The part of a symbol before the '-': "BTC-USD" -> "BTC". If there's no
/// '-', return the whole input. Return a slice of the input (no allocation;
/// the test checks the pointer).
/// Hint: `s.find('-')` returns Option<usize>, a byte index.
#[allow(dead_code)]
fn base_asset(symbol: &str) -> &str {
    if let Some((asset, _)) = symbol.split_once('-') {
        asset
    } else {
        symbol
    }
}

/// Trade two orders against each other: traded = the smaller qty. Subtract
/// it from both and return it.
#[allow(dead_code)]
fn fill_against(taker: &mut Order, maker: &mut Order) -> u64 {
    let traded_qty = taker.qty.min(maker.qty);
    taker.qty -= traded_qty;
    maker.qty -= traded_qty;
    traded_qty
}

/// Same as fill_against, but both orders sit in one slice at indexes `i`
/// and `j`. They're always different, but `i` may be bigger than `j`. Pass
/// book[i] as the taker.
/// Hint: split_at_mut, then reuse fill_against.
#[allow(dead_code)]
fn cross_in_place(book: &mut [Order], i: usize, j: usize) -> u64 {
    if i > j {
        let (left, right) = book.split_at_mut(i);
        fill_against(&mut right[0], &mut left[j])
    } else {
        let (left, right) = book.split_at_mut(j);
        fill_against(&mut left[i], &mut right[0])
    }
}

/// Remove every order that belongs to `client` and return how many were
/// removed.
/// Hint: `orders.retain(|o| ...)` keeps only the elements for which the
/// closure returns true (closures are covered in phase 6). Compare lengths
/// before and after.
#[allow(dead_code)]
fn cancel_all_for(orders: &mut Vec<Order>, client: &str) -> usize {
    let mut order_ids_to_remove: Vec<u64> = Vec::new();
    for (i, order) in orders.iter().enumerate() {
        if order.client == client {
            order_ids_to_remove.push(order.id);
        }
    }

    orders.retain(|o| !order_ids_to_remove.contains(&o.id));
    order_ids_to_remove.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[Order]) -> Vec<u64> {
        v.iter().map(|o| o.id).collect()
    }

    fn qtys(v: &[Order]) -> Vec<u64> {
        v.iter().map(|o| o.qty).collect()
    }

    #[test]
    fn t_total_qty() {
        let v = vec![
            order(1, Side::Buy, 100, 5, "a"),
            order(2, Side::Sell, 101, 7, "b"),
        ];
        assert_eq!(total_qty(&v), 12);
        assert_eq!(total_qty(&v[1..]), 7);
        assert_eq!(total_qty(&[]), 0);
    }

    #[test]
    fn t_best_ask() {
        let asks = vec![
            order(1, Side::Sell, 102, 1, "a"),
            order(2, Side::Sell, 101, 1, "b"),
            order(3, Side::Sell, 101, 1, "c"),
        ];
        let best = best_ask(&asks).unwrap();
        assert_eq!(best.id, 2);
        assert!(std::ptr::eq(best, &asks[1])); // points into the slice, not a copy
        assert!(best_ask(&[]).is_none());
    }

    #[test]
    fn t_base_asset() {
        let s = String::from("BTC-USD");
        let b = base_asset(&s);
        assert_eq!(b, "BTC");
        assert_eq!(b.as_ptr(), s.as_ptr()); // same memory as the input
        assert_eq!(base_asset("SOL"), "SOL");
    }

    #[test]
    fn t_fill_against() {
        let mut t = order(1, Side::Buy, 100, 5, "a");
        let mut m = order(2, Side::Sell, 100, 3, "b");
        assert_eq!(fill_against(&mut t, &mut m), 3);
        assert_eq!((t.qty, m.qty), (2, 0));
    }

    #[test]
    fn t_cross_in_place() {
        let mut book = vec![
            order(1, Side::Buy, 100, 5, "a"),
            order(2, Side::Sell, 100, 10, "b"),
            order(3, Side::Buy, 100, 4, "c"),
        ];
        assert_eq!(cross_in_place(&mut book, 2, 1), 4); // i > j works too
        assert_eq!(qtys(&book), vec![5, 6, 0]);
        assert_eq!(cross_in_place(&mut book, 0, 1), 5);
        assert_eq!(qtys(&book), vec![0, 1, 0]);
    }

    #[test]
    fn t_cancel_all_for() {
        let mut v = vec![
            order(1, Side::Buy, 100, 1, "alice"),
            order(2, Side::Buy, 100, 1, "bob"),
            order(3, Side::Sell, 101, 1, "alice"),
            order(4, Side::Sell, 101, 1, "carol"),
        ];
        assert_eq!(cancel_all_for(&mut v, "alice"), 2);
        assert_eq!(ids(&v), vec![2, 4]);
        assert_eq!(cancel_all_for(&mut v, "zed"), 0);
    }
}
