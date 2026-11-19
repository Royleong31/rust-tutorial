//! PHASE 3 — Ownership
//!
//!   cargo run  --bin p03_ownership
//!   cargo test --bin p03_ownership
//!
//! THE RULES (there are only three):
//!   1. Every value has exactly one OWNER (a variable, a struct field, a Vec slot...).
//!   2. Ownership can be MOVED to a new owner. The old one becomes unusable.
//!   3. When the owner goes out of scope, the value is DROPPED (freed) at
//!      once, deterministically. There's no garbage collector.
//!
//! In JS, `const b = a` (for objects) gives you two handles to the same heap
//! object, and the GC frees it when nobody points at it any more. Rust has
//! no GC, so it needs exactly one party responsible for freeing each heap
//! allocation. Most of the rules below follow from that.
//!
//! Go slowly here. Uncomment EVERY ❌ block and read the full error, because
//! these are the errors you'll hit most in your first month.

use std::mem;

fn main() {
    section_stack_vs_heap();
    section_move();
    section_copy();
    section_clone();
    section_functions_and_ownership();
    section_moves_in_loops_and_matches();
    section_moving_out_of_collections();
    section_drop_and_raii();
    puzzles();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Buy,
    Sell,
}

// `client: String` owns heap memory, so Order CANNOT be Copy. That's
// deliberate: this struct shows how ownership behaves.
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
// 1. Stack vs heap: what actually lives where
// ─────────────────────────────────────────────────────────────────────────────
//
//   STACK: fixed-size values whose size is known at compile time. Allocating
//          is just bumping a pointer, and they're freed automatically when
//          the function returns. Integers, bools, chars, fixed arrays, and
//          structs made only of those live here.
//
//   HEAP:  memory whose size is only known at runtime, or that grows:
//          String contents, Vec elements, Box<T>. Allocating means calling
//          the allocator, which is slower. SOMEONE has to free it.
//
//   A `String` is a small fixed-size HEADER on the stack, 24 bytes on 64-bit:
//
//        stack (the `String` value)         heap
//        ┌──────────┬─────┬──────────┐      ┌───┬───┬───┬───┬───┐
//        │ ptr ─────┼─────┼──────────┼────► │ h │ e │ l │ l │ o │
//        │ len = 5  │     │ cap = 5  │      └───┴───┴───┴───┴───┘
//        └──────────┴─────┴──────────┘
//
//   Vec<T> has the same layout: (ptr, len, capacity).

fn section_stack_vs_heap() {
    println!("\n== stack vs heap ==");
    println!("size_of i64        = {}", mem::size_of::<i64>());
    println!("size_of Side       = {}", mem::size_of::<Side>()); // a fieldless enum is 1 byte
    println!("size_of String     = {}", mem::size_of::<String>()); // 24: just the header
    println!("size_of Vec<u64>   = {}", mem::size_of::<Vec<u64>>()); // 24, whatever the length
    println!("size_of Order      = {}", mem::size_of::<Order>()); // client's text is NOT counted

    let s = String::from("hello");
    println!("header lives at {:p}, text lives at {:p}", &s, s.as_ptr());
    // The two addresses are far apart: one is in the stack region, the other
    // in the heap region.
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Move
// ─────────────────────────────────────────────────────────────────────────────

fn section_move() {
    println!("\n== move ==");

    let a = String::from("hello");
    let a_heap = a.as_ptr();

    let b = a; // MOVE: copies the 24-byte header. The heap text is NOT copied.

    println!(
        "b's text at {:p} (a's was {:p}): same allocation",
        b.as_ptr(),
        a_heap
    );

    // Why is `a` now dead? If both a and b were usable, then at the end of
    // scope BOTH would try to free the same heap buffer (a double free:
    // memory corruption, a classic C bug). Rust's answer is that only one
    // can own it. After the move the compiler treats `a` as uninitialised.
    //
    //   a ──X
    //   b ────► "hello"
    //
    // ❌ UNCOMMENT -> error[E0382]: borrow of moved value: `a`
    //   Read the WHOLE error. It points at where the move happened
    //   ("value moved here") and even suggests `.clone()`.
    // println!("{a}");

    // A move is a shallow memcpy of the header, so it's cheap. Moving a
    // Vec of a million orders copies 24 bytes. No deep copy ever happens
    // unless you ask for one.

    // Struct with heap fields: same thing.
    let o1 = order(1, Side::Buy, 100, 5, "alice");
    let o2 = o1;
    println!("o2 = {o2:?}");
    // ❌ UNCOMMENT -> error[E0382]: borrow of moved value: `o1`
    // println!("{:?}", o1);

    // You CAN reuse a moved-from variable by giving it a new value.
    let mut c = String::from("x");
    let d = c;
    c = String::from("y"); // fine: c owns something new
    println!("c={c} d={d}");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Copy: types that are "just bits"
// ─────────────────────────────────────────────────────────────────────────────
//
// If a type owns NO heap resources, copying its bytes gives a complete,
// independent value, so there's no double-free risk. Such types can be
// `Copy`, and assigning one copies instead of moving. The original stays
// usable.
//
//   Copy:     integers, floats, bool, char, &T (shared references),
//             tuples/arrays of Copy types, and your structs/enums IF you
//             #[derive(Clone, Copy)] and all their fields are Copy.
//   NOT Copy: String, Vec<T>, Box<T>, anything with a Drop impl, &mut T.

#[derive(Debug, Clone, Copy)]
struct Quote {
    bid: i64,
    ask: i64,
}

fn section_copy() {
    println!("\n== copy ==");
    let x = 5;
    let y = x; // copy
    println!("x={x} y={y}"); // both fine

    let q1 = Quote { bid: 99, ask: 101 };
    let mut q2 = q1; // copy, because Quote is Copy
    q2.bid = 100;
    println!("q1={q1:?} q2={q2:?} q1.ask={}", q1.ask); // independent values

    let side = Side::Buy;
    let side2 = side;
    println!("{side:?} {side2:?}");

    // ❌ UNCOMMENT -> error[E0204]: the trait `Copy` cannot be implemented for this type
    //   (`String` is not Copy, so a struct containing one can't be either)
    // #[derive(Clone, Copy)]
    // struct Bad { name: String }

    // DESIGN TIP for your exchange: make small value types (Price, Qty,
    // OrderId, Side) Copy and pass them around freely. Keep big or
    // heap-owning things (Order, OrderBook) non-Copy so moves stay visible.
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Clone: explicit deep copy
// ─────────────────────────────────────────────────────────────────────────────

fn section_clone() {
    println!("\n== clone ==");
    let a = String::from("hello");
    let b = a.clone(); // allocates a NEW heap buffer and copies the bytes
    println!(
        "a at {:p}, b at {:p}: different allocations",
        a.as_ptr(),
        b.as_ptr()
    );
    println!("a={a} b={b}"); // both usable

    // Clone is always explicit in Rust, so a reader can spot every
    // potentially expensive copy. JS hides this: structuredClone(),
    // [...arr], and {...obj} all look cheap.
    //
    // COMMON BEGINNER TRAP: sprinkling .clone() everywhere to make the
    // compiler happy. It works, but it usually means you wanted a BORROW
    // (phase 4). Rule of thumb while learning: if you clone to fix an error,
    // stop and ask whether the callee really needs to own the value.

    // Copy vs Clone:
    //   Copy  = implicit, always a cheap bitwise copy, can't be customised
    //   Clone = explicit .clone(), may be expensive, can have custom logic
    //   Every Copy type is also Clone.
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Functions take and give ownership
// ─────────────────────────────────────────────────────────────────────────────

// Takes ownership. The caller's value is moved INTO `o`, and when this
// function returns, `o` goes out of scope and is dropped (freed).
fn submit_and_forget(o: Order) {
    println!("  submitted #{} for {}", o.id, o.client);
} // <- `o` dropped here. Its `client` String is freed.

// Takes ownership and gives it back. This works but it's clumsy, and it's
// exactly what borrowing (&Order) fixes in phase 4.
fn submit_and_return(o: Order) -> Order {
    println!("  submitted #{} (returning it)", o.id);
    o // moved back out to the caller
}

// Taking a Copy type "by value" just copies it, so nothing is lost.
fn double(px: i64) -> i64 {
    px * 2
}

fn section_functions_and_ownership() {
    println!("\n== functions ==");

    let o = order(1, Side::Buy, 100, 5, "alice");
    submit_and_forget(o);
    // ❌ UNCOMMENT -> error[E0382]: borrow of moved value: `o`
    // println!("{:?}", o);

    let o = order(2, Side::Sell, 101, 3, "bob");
    let o = submit_and_return(o); // shadowing: take ownership back
    println!("  still have #{}", o.id);

    let px = 100;
    println!("  double={} px still={px}", double(px));

    // Methods follow the same rules through their receiver:
    //   fn f(&self)      borrows
    //   fn f(&mut self)  borrows mutably
    //   fn f(self)       TAKES OWNERSHIP (consumes). Examples in std:
    //                    String::into_bytes, Vec::into_iter, Option::unwrap
    let s = String::from("BTC-USD");
    let bytes = s.into_bytes(); // s is consumed
    println!("  bytes len {}", bytes.len());
    // ❌ UNCOMMENT -> error[E0382]: borrow of moved value: `s`
    // println!("{s}");
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Moves inside loops and matches: the sneaky ones
// ─────────────────────────────────────────────────────────────────────────────

fn section_moves_in_loops_and_matches() {
    println!("\n== loops & matches ==");

    let orders = vec![
        order(1, Side::Buy, 100, 5, "alice"),
        order(2, Side::Sell, 101, 2, "bob"),
    ];

    // `for x in vec` CONSUMES the Vec. Each element is moved out to `x` in
    // turn, and the Vec is gone afterwards. (Under the hood this is
    // vec.into_iter(), covered in phase 6.)
    for o in orders {
        println!("  consumed #{}", o.id);
    }
    // ❌ UNCOMMENT -> error[E0382]: borrow of moved value: `orders`
    //   ("`orders` moved due to this implicit call to `.into_iter()`")
    // println!("{}", orders.len());
    //   Fix preview: `for o in &orders` iterates by reference, leaving
    //   `orders` intact.

    // Moving inside a loop body:
    let report = String::from("EOD report");
    for day in 0..2 {
        let _ = day;
        // ❌ UNCOMMENT -> error[E0382]: use of moved value: `report`
        //   ("value moved here, in previous iteration of loop")
        // submit_report(report);
    }
    let _ = report;

    // PARTIAL MOVES: moving one field out of a struct.
    let o = order(3, Side::Buy, 99, 1, "carol");
    let client = o.client; // moves only the String field out
    println!("  client={client}, id still readable: {}", o.id); // Copy fields are fine
    // ❌ UNCOMMENT -> error[E0382]: borrow of partially moved value: `o`
    // println!("{:?}", o);

    // match / if let can move as well:
    let maybe: Option<String> = Some(String::from("cancel-reason"));
    if let Some(reason) = maybe {
        // `reason` took ownership of the String inside
        println!("  reason={reason}");
    }
    // ❌ UNCOMMENT -> error[E0382]: borrow of partially moved value: `maybe`
    // println!("{:?}", maybe);
    //   Fix preview: `if let Some(reason) = &maybe` borrows instead of moving.
}

#[allow(dead_code)]
fn submit_report(r: String) {
    let _ = r;
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Moving OUT of a collection: you can't just "take" an element
// ─────────────────────────────────────────────────────────────────────────────
//
// If `let o = book[0];` were allowed, slot 0 of the Vec would be left
// holding a moved-from value, and the Vec would later try to drop it again.
// So Rust forbids it. You have to say what happens to the slot.

fn section_moving_out_of_collections() {
    println!("\n== moving out of collections ==");
    let mut book = vec![
        order(1, Side::Buy, 100, 5, "alice"),
        order(2, Side::Buy, 99, 5, "bob"),
        order(3, Side::Buy, 98, 5, "carol"),
    ];

    // ❌ UNCOMMENT -> error[E0507]: cannot move out of index of `Vec<Order>`
    // let first = book[0];

    // Options, each with a different cost and meaning:
    let copy = book[0].clone(); // 1. deep copy; the Vec is unchanged
    let removed = book.remove(0); // 2. take it out, shift the rest left (O(n))
    let swapped = book.swap_remove(0); // 3. take it out, move the last element into the hole (O(1), changes order!)
    println!(
        "  clone=#{} removed=#{} swap_removed=#{}",
        copy.id, removed.id, swapped.id
    );
    println!(
        "  left in book: {:?}",
        book.iter().map(|o| o.id).collect::<Vec<_>>()
    );

    // 4. mem::replace / mem::take: swap a value in and get the old one out.
    //    `take` leaves Default::default() behind (empty String, empty Vec...).
    let mut o = order(9, Side::Sell, 105, 1, "dave");
    let old_client = mem::take(&mut o.client); // o.client is now ""
    let older = mem::replace(&mut o.client, String::from("eve"));
    println!("  took {old_client:?}, then {older:?}; now {:?}", o.client);

    // 5. Option::take: the standard way to move out of an Option field.
    //    Leaves None behind. You'll use this ALL the time for state machines.
    let mut pending: Option<Order> = Some(order(10, Side::Buy, 1, 1, "frank"));
    let p = pending.take();
    println!("  took {:?}, pending is now {:?}", p.map(|o| o.id), pending);
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Drop and RAII: cleanup tied to scope
// ─────────────────────────────────────────────────────────────────────────────
//
// RAII ("Resource Acquisition Is Initialisation"): a value that holds a
// resource (memory, a file, a socket, a lock, a risk reservation) releases
// it in its destructor, and the destructor runs automatically when the
// owner goes out of scope.
//
// TS equivalents are try/finally, or `using x = ...` with [Symbol.dispose]
// from TS 5.2. In Rust EVERY value works this way, and you can't forget it.

struct RiskReservation {
    client: String,
    notional: i64,
}

impl RiskReservation {
    fn reserve(client: &str, notional: i64) -> Self {
        println!("  + reserve {notional} for {client}");
        RiskReservation {
            client: client.to_string(),
            notional,
        }
    }
}

// The Drop trait is Rust's destructor. You almost never call it directly.
impl Drop for RiskReservation {
    fn drop(&mut self) {
        println!("  - release {} for {}", self.notional, self.client);
    }
}

fn section_drop_and_raii() {
    println!("\n== drop / RAII ==");
    // 🔮 PREDICT the output order before you run this. Write it down!

    let _a = RiskReservation::reserve("alice", 100);
    {
        let _b = RiskReservation::reserve("bob", 200);
        println!("  inner scope ending");
    } // _b dropped HERE, deterministically
    let c = RiskReservation::reserve("carol", 300);
    let d = RiskReservation::reserve("dave", 400);

    drop(c); // explicit early drop. std's `drop` is just `fn drop<T>(_x: T) {}`:
    //          it takes ownership and lets the value go out of scope. Neat.

    let moved = d; // moving does NOT drop; responsibility just moves to `moved`
    let _ = &moved;

    // Assigning to `_` with no binding drops IMMEDIATELY. A classic bug
    // with lock guards:
    let _ = RiskReservation::reserve("ghost", 1); // released on this same line!

    println!("  end of function");
} // remaining values dropped in REVERSE declaration order: moved(dave), then _a(alice)

// Notes:
//  - Drop order matters for real resources. A DB transaction guard should
//    drop before the connection it uses, and Rust's reverse order handles
//    that naturally.
//  - A type with Drop can't be Copy (one destructor per value).
//  - Values are also dropped during a panic unwind, so cleanup still happens.
//  - mem::forget(x) skips the destructor. You'll almost never want that.

// ─────────────────────────────────────────────────────────────────────────────
// 9. PUZZLES. Each block below has a compile error. Uncomment one, read the
//    error, fix it in the most sensible way (NOT always .clone()!), and ask
//    me if the "why" isn't clear.
// ─────────────────────────────────────────────────────────────────────────────
fn puzzles() {
    println!("\n== puzzles (uncomment in source) ==");

    // PUZZLE 1: log a client name, then store it in an order.
    // let client = String::from("alice");
    // let o = order(1, Side::Buy, 100, 1, "x");
    // let o = Order { client, ..o };
    // println!("created order for {client}");

    // PUZZLE 2: print every order's id, then report how many there were.
    // let orders = vec![order(1, Side::Buy, 1, 1, "a"), order(2, Side::Sell, 1, 1, "b")];
    // for o in orders { println!("{}", o.id); }
    // println!("total = {}", orders.len());

    // PUZZLE 3: send the same order to two venues.
    // fn send(venue: &str, o: Order) { println!("{venue} <- #{}", o.id); }
    // let o = order(1, Side::Buy, 100, 1, "alice");
    // send("NYSE", o);
    // send("NASDAQ", o);

    // PUZZLE 4: take the client name out of the first order in a Vec.
    // let mut v = vec![order(1, Side::Buy, 100, 1, "alice")];
    // let name: String = v[0].client;
    // println!("{name}");
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES: `cargo test --bin p03_ownership`
// Rule: no .clone() calls allowed in your solutions. Use moves.
// ═════════════════════════════════════════════════════════════════════════════

/// Split orders into (buys, sells), keeping relative order. Consumes the input.
/// Hint: `for o in orders { ... }` moves each order out; push it into one of
/// two new Vecs.
#[allow(dead_code)]
fn partition_by_side(orders: Vec<Order>) -> (Vec<Order>, Vec<Order>) {
    let mut buys: Vec<Order> = Vec::new();
    let mut sells: Vec<Order> = Vec::new();

    for o in orders {
        match o.side {
            Side::Buy => buys.push(o),
            Side::Sell => sells.push(o),
        }
    }

    (buys, sells)
}

/// Remove and return the HIGHEST-priced order from `book` (the first one if
/// prices tie). Return None if the book is empty. Must not clone.
/// Hint: find the index with a loop, then use one of the "moving out" tools
/// from section 7. Which one keeps the remaining orders in order?
#[allow(dead_code)]
fn pop_best_bid(book: &mut Vec<Order>) -> Option<Order> {
    let mut highest_price_index = 0;

    if book.is_empty() {
        return None;
    }

    for (i, order) in book.iter().enumerate() {
        if order.price > book[highest_price_index].price {
            highest_price_index = i;
        }
    }

    Some(book.remove(highest_price_index))
}

/// Return the order with its client renamed, plus the OLD client name.
/// Hint: mem::replace.
#[allow(dead_code)]
fn reassign(mut o: Order, new_client: &str) -> (Order, String) {
    let old_client = mem::take(&mut o.client); // o.client is now ""
    let _ = mem::replace(&mut o.client, String::from(new_client));

    (o, old_client)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[Order]) -> Vec<u64> {
        v.iter().map(|o| o.id).collect()
    }

    #[test]
    fn t_partition_by_side() {
        let orders = vec![
            order(1, Side::Buy, 100, 1, "a"),
            order(2, Side::Sell, 101, 1, "b"),
            order(3, Side::Buy, 99, 1, "c"),
        ];
        let (buys, sells) = partition_by_side(orders);
        assert_eq!(ids(&buys), vec![1, 3]);
        assert_eq!(ids(&sells), vec![2]);
    }

    #[test]
    fn t_pop_best_bid() {
        let mut book = vec![
            order(1, Side::Buy, 99, 1, "a"),
            order(2, Side::Buy, 101, 1, "b"),
            order(3, Side::Buy, 101, 1, "c"),
            order(4, Side::Buy, 100, 1, "d"),
        ];
        assert_eq!(pop_best_bid(&mut book).map(|o| o.id), Some(2));
        assert_eq!(ids(&book), vec![1, 3, 4]); // order preserved!
        assert_eq!(pop_best_bid(&mut book).map(|o| o.id), Some(3));
        let mut empty = vec![];
        assert!(pop_best_bid(&mut empty).is_none());
    }

    #[test]
    fn t_reassign() {
        let (o, old) = reassign(order(1, Side::Buy, 100, 1, "alice"), "bob");
        assert_eq!(o.client, "bob");
        assert_eq!(old, "alice");
    }
}
