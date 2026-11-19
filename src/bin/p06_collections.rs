//! PHASE 6 — Collections, closures and iterators
//!
//!   cargo run  --bin p06_collections
//!   cargo test --bin p06_collections
//!
//! Your order book is built out of these:
//!
//!   String / &str   owned text / borrowed view of text
//!   Vec<T>          growable array (JS Array)
//!   VecDeque<T>     ring buffer, O(1) at both ends -> the FIFO queue at one price
//!   HashMap<K, V>   unordered lookup (JS Map)      -> order id -> where it rests
//!   BTreeMap<K, V>  SORTED map                     -> price levels, best bid/ask
//!   HashSet<T> / BTreeSet<T>                       -> unique clients, symbols...
//!
//! Plus closures and iterators, which are how you work with all of them.
//! Everything from phases 3–5 applies: collections OWN their elements,
//! `.iter()` borrows, `.into_iter()` consumes.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::hash::Hash;

fn main() {
    section_strings();
    section_vec();
    section_closures();
    section_iterators();
    section_hashmap();
    section_btreemap();
    section_vecdeque();
    section_sets();
    section_order_book();
    puzzles();
}

// `Hash` lets Side be a HashMap key (section 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

fn sample_orders() -> Vec<Order> {
    vec![
        order(1, Side::Buy, 100, 5, "alice"),
        order(2, Side::Sell, 103, 2, "bob"),
        order(3, Side::Buy, 101, 1, "carol"),
        order(4, Side::Buy, 101, 4, "alice"),
        order(5, Side::Sell, 102, 7, "dave"),
    ]
}

fn ids(orders: &[Order]) -> Vec<u64> {
    orders.iter().map(|o| o.id).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. String, &str and str
// ─────────────────────────────────────────────────────────────────────────────
//
// JS has one string type. Rust has two you use every day:
//
//   String  OWNED, growable, heap-allocated UTF-8 text: (ptr, len, cap), 24 bytes.
//           Think of it as a Vec<u8> that's guaranteed to be valid UTF-8.
//   &str    a BORROWED view of UTF-8 bytes that live somewhere else:
//           (ptr, len), 16 bytes. It can point into a String, into the
//           compiled binary (string literals), or into a network buffer
//           (phase 5's FIX parser).
//
//   str     the text bytes themselves, with no pointer attached. Their size
//           isn't known at compile time (a "dynamically sized type", phase 9),
//           so you never hold a bare `str`, only &str, Box<str> and so on.
//
// Collections follow the same pattern: Vec<T> owns, &[T] borrows, and [T] is
// the unsized run of elements.
//
// Which one to use:
//   struct fields, anything you keep    -> String
//   parameters you only read            -> &str
//   returning a string you built        -> String
//   returning part of an input          -> &str (phase 5)

fn shout(s: &str) -> String {
    s.to_uppercase() // builds a NEW String
}

fn section_strings() {
    println!("\n== String / &str ==");

    // Ways to make a String. Every one of them allocates:
    let a = String::from("BTC");
    let b = "ETH".to_string();
    let c: String = "SOL".into();
    let pair = format!("{a}-USD"); // the most common one; like a JS template string
    println!("  {a} {b} {c} {pair}");

    // String -> &str is free (nothing is copied): write &s or s.as_str(), or
    // just pass &s where a &str is expected.
    let view: &str = &pair;
    println!("  shout({view}) = {}", shout(view));
    println!("  shout(literal) = {}", shout("doge")); // a literal is a &'static str

    // Growing a String. It needs `mut`, and only an owned String can grow:
    let mut log = String::with_capacity(64);
    log.push_str("fill ");
    log.push('#');
    log += "42"; // += takes a &str
    println!("  log = {log:?} len={} cap={}", log.len(), log.capacity());

    // `+` TAKES OWNERSHIP of the left side and reuses its buffer:
    let left = String::from("BTC");
    let joined = left + "-" + "USD"; // left is moved into joined
    println!("  joined = {joined}");
    // ❌ UNCOMMENT -> error[E0382]: borrow of moved value: `left`
    // println!("{left}");
    // format!("{left}-USD") only borrows, so it leaves `left` usable.

    // len() counts BYTES, not characters. A non-ASCII character takes 2–4 bytes:
    let label = "€100";
    println!(
        "  {label:?}: {} bytes, {} chars",
        label.len(),
        label.chars().count()
    );

    // You can't index a string by position, because a byte position can land
    // in the middle of a character:
    // ❌ UNCOMMENT -> error[E0277]: the type `str` cannot be indexed by `{integer}`
    // let first = label[0];
    // Use label.chars().next(), label.chars().nth(i), or a byte RANGE that
    // falls on character boundaries (phase 4, section 6).
    println!("  first char = {:?}", label.chars().next());

    // Everyday &str methods. These return views into the original; nothing is copied:
    let raw = "  BTC-USD , ETH-USD ,SOL-USD ";
    let symbols: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
    println!("  symbols = {symbols:?}");
    println!(
        "  starts_with BTC? {}  strip_suffix: {:?}  parse: {:?}",
        symbols[0].starts_with("BTC"),
        symbols[0].strip_suffix("-USD"),
        "6500025".parse::<i64>(),
    );

    // A struct that KEEPS a name owns it as a String. Turning a &str into a
    // String allocates, so do it once, when you store the value (phase 5,
    // section 7).
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Vec<T> in more depth
// ─────────────────────────────────────────────────────────────────────────────

fn section_vec() {
    println!("\n== Vec ==");

    // len = how many elements are in it. capacity = how many fit before it
    // has to reallocate (allocate a bigger buffer and move everything over).
    let mut v: Vec<u64> = Vec::new();
    let mut last_cap = v.capacity();
    for i in 0..20 {
        v.push(i);
        if v.capacity() != last_cap {
            println!(
                "  len {:>2} -> capacity {:>2} (reallocated)",
                v.len(),
                v.capacity()
            );
            last_cap = v.capacity();
        }
    }
    // If you know the size up front, allocate once:
    let pre: Vec<Order> = Vec::with_capacity(1_000);
    println!(
        "  with_capacity(1000): len {} capacity {}",
        pre.len(),
        pre.capacity()
    );

    v.truncate(3);
    println!("  pop -> {:?}, left {v:?}", v.pop()); // pop returns an Option
    v.insert(0, 99); // O(n): shifts everything right
    println!(
        "  after insert(0, 99): {v:?}, contains 99? {}",
        v.contains(&99)
    );

    // SORTING. sort_by_key sorts by whatever the closure returns. It's
    // STABLE: elements with equal keys keep their original order.
    let mut book = sample_orders();
    book.sort_by_key(|o| o.price);
    println!("  by price asc:          {:?}", ids(&book));

    // Price-time priority for BIDS: highest price first, then oldest (lowest
    // id) first. sort_by takes a comparator, like JS's arr.sort((a, b) => ...),
    // but it returns an Ordering instead of a number.
    book.sort_by(|a, b| b.price.cmp(&a.price).then(a.id.cmp(&b.id)));
    println!("  by price desc, id asc: {:?}", ids(&book));
    // Same order, using a tuple key. Tuples compare field by field, and
    // Reverse flips one field:
    book.sort_by_key(|o| (Reverse(o.price), o.id));
    println!("  same, via Reverse:     {:?}", ids(&book));

    // ❌ UNCOMMENT -> error[E0277]: the trait bound `f64: Ord` is not satisfied
    // let mut pxs: Vec<f64> = vec![1.5, 0.5];
    // pxs.sort();

    // f64 isn't `Ord` because NaN can't be compared with anything. Use
    // `pxs.sort_by(|a, b| a.total_cmp(b))`, or better, use integer ticks.

    // binary_search on a SORTED Vec or array: Ok(index) if found, Err(index
    // where it would go) if not.
    let levels = [100, 101, 103, 104];
    println!(
        "  search 103: {:?}, search 102: {:?}",
        levels.binary_search(&103),
        levels.binary_search(&102)
    );

    // retain keeps only what the closure accepts. dedup removes CONSECUTIVE
    // duplicates. drain removes a range and hands the removed items to you.
    book.retain(|o| o.qty > 1);
    println!("  retain qty > 1: {:?}", ids(&book));
    let mut seq = vec![1, 1, 2, 3, 3, 3, 1];
    seq.dedup();
    println!("  dedup: {seq:?}"); // the final 1 stays: it isn't next to the others
    let first_two: Vec<Order> = book.drain(..2).collect();
    println!("  drained {:?}, left {:?}", ids(&first_two), ids(&book));
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Closures
// ─────────────────────────────────────────────────────────────────────────────

// A function that takes a closure. `F: Fn(i64) -> i64` reads "F is something
// callable that takes an i64 and returns an i64". It's a generic, so each
// call site gets its own compiled copy and the closure can be inlined.
fn apply_to_all<F: Fn(i64) -> i64>(pxs: &[i64], f: F) -> Vec<i64> {
    let mut out = Vec::with_capacity(pxs.len());
    for &p in pxs {
        out.push(f(p));
    }
    out
}

// Returning a closure. `impl Fn(i64) -> i64` means "some closure type, I
// won't name it". `move` puts `bps` inside the closure, since this function's
// local variables are gone once it returns.
fn fee_model(bps: i64) -> impl Fn(i64) -> i64 {
    move |notional| notional * bps / 10_000
}

fn section_closures() {
    println!("\n== closures ==");

    // |args| body. Argument and return types are usually inferred.
    let add_tick = |px: i64| px + 1;
    let tick_size = 5;
    let round_to_tick = |px: i64| px / tick_size * tick_size; // uses `tick_size` from outside
    println!(
        "  add_tick(100)={} round_to_tick(103)={}",
        add_tick(100),
        round_to_tick(103)
    );
    println!(
        "  rounded: {:?}",
        apply_to_all(&[101, 107, 112], round_to_tick)
    );

    // Like JS arrow functions, closures CAPTURE variables from the
    // surrounding scope. Unlike JS, the compiler picks how, using the least
    // it needs: a shared borrow, a mutable borrow, or a move.
    let mut fills = 0;
    let mut record_fill = || fills += 1; // captures `fills` by &mut, so the closure must be `mut`
    record_fill();
    record_fill();
    println!("  fills = {fills}"); // the &mut borrow ended at record_fill's last use

    // While the closure holds that &mut, nothing else can read the variable:
    // ❌ UNCOMMENT -> error[E0502]: cannot borrow `count` as immutable because it is also borrowed as mutable
    // let mut count = 0;
    // let mut inc = || count += 1;
    // println!("{count}");
    // inc();

    // `move` makes the closure take OWNERSHIP of what it captures. You need
    // it when the closure outlives the current scope: returning it (see
    // fee_model above) or sending it to a thread (phase 10).
    let client = String::from("alice");
    let tag = move |id: u64| format!("{client}#{id}");
    println!("  {}", tag(7));
    // ❌ UNCOMMENT -> error[E0382]: borrow of moved value: `client`
    // println!("{client}");

    let taker_fee = fee_model(5);
    println!("  fee on 1,000,000 = {}", taker_fee(1_000_000));

    // Three closure traits. The compiler decides which ones a closure
    // implements by looking at its body:
    //   FnOnce  can be called once (it may move its captures out)
    //   FnMut   can be called many times, may change its captures
    //   Fn      can be called many times, only reads its captures
    // Every Fn closure is also FnMut, and every FnMut is also FnOnce.
    //
    // When YOUR function takes a closure, ask for the loosest bound your code
    // needs, so callers can pass the most kinds of closure: FnOnce if you call
    // it once, FnMut if you call it repeatedly, Fn only if you must call it
    // through a shared reference (for example from several threads).
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Iterators
// ─────────────────────────────────────────────────────────────────────────────
//
// An iterator is anything with `fn next(&mut self) -> Option<Item>`.
// ADAPTERS (map, filter, zip...) are LAZY: they only build a pipeline.
// Nothing runs until a CONSUMER (sum, count, collect, a for loop...) pulls
// values through, one element at a time and in a single pass. JS's
// arr.map().filter() builds a whole new array at each step instead.
//
// Where iterators come from:
//   v.iter()       yields &T       (what `for x in &v` uses)
//   v.iter_mut()   yields &mut T   (`for x in &mut v`)
//   v.into_iter()  yields T, consumes v (`for x in v`)

fn section_iterators() {
    println!("\n== iterators ==");
    let book = sample_orders();

    // Proof that adapters are lazy:
    let pipeline = book.iter().map(|o| {
        println!("    visiting #{}", o.id);
        o.qty
    });
    println!("  pipeline built, nothing visited yet");
    let total: u64 = pipeline.take(2).sum(); // take(2): only 2 elements get pulled
    println!("  sum of first 2 qtys = {total}");

    // The everyday ones. Inside `filter`, the closure gets `&&Order` (a
    // reference to the item, which is itself a reference). `.` derefs both
    // levels, so `o.side` just works.
    let buy_qty: u64 = book
        .iter()
        .filter(|o| o.side == Side::Buy)
        .map(|o| o.qty)
        .sum();
    let n_sells = book.iter().filter(|o| o.side == Side::Sell).count();
    println!("  buy qty={buy_qty} sells={n_sells}");

    // min_by_key returns the FIRST minimum, but max_by_key returns the LAST
    // maximum. For "best bid, oldest first" use min_by_key with Reverse:
    let best_bid = book
        .iter()
        .filter(|o| o.side == Side::Buy)
        .min_by_key(|o| (Reverse(o.price), o.id));
    let last_max = book
        .iter()
        .filter(|o| o.side == Side::Buy)
        .max_by_key(|o| o.price);
    println!(
        "  best bid {:?}, but max_by_key picks {:?}",
        best_bid.map(|o| o.id),
        last_max.map(|o| o.id)
    );

    println!(
        "  any qty>6: {}  all price>0: {}  position(bob): {:?}  find(carol): {:?}",
        book.iter().any(|o| o.qty > 6),
        book.iter().all(|o| o.price > 0),
        book.iter().position(|o| o.client == "bob"),
        book.iter().find(|o| o.client == "carol").map(|o| o.id),
    );

    // Combining sequences:
    let prices = [100, 101, 103];
    let qtys = [5, 2, 7];
    let notional: i64 = prices.iter().zip(qtys.iter()).map(|(p, q)| p * q).sum();
    let changes: Vec<i64> = prices.windows(2).map(|w| w[1] - w[0]).collect();
    // fold is JS's reduce. Here the accumulator is a tuple: (position, peak position).
    let (position, peak) = [5, -2, 6, -9]
        .iter()
        .fold((0, 0), |(pos, peak), q| (pos + q, peak.max(pos + q)));
    println!("  notional={notional} changes={changes:?} position={position} peak={peak}");
    print!(" ");
    for (i, px) in prices.iter().enumerate().rev().skip(1) {
        print!(" [{i}]={px}");
    }
    println!();

    // collect() can build many different collections. The type you ask for
    // decides which one:
    let id_list: Vec<u64> = book.iter().map(|o| o.id).collect();
    let clients: HashSet<&str> = book.iter().map(|o| o.client.as_str()).collect();
    let by_id: HashMap<u64, &Order> = book.iter().map(|o| (o.id, o)).collect();
    let joined: String = id_list
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "  ids={id_list:?} clients={} by_id[3]={} joined={joined}",
        clients.len(),
        by_id[&3].client
    );

    // ❌ UNCOMMENT -> error[E0283]: type annotations needed
    // let guess = book.iter().map(|o| o.id).collect();
    // println!("{guess:?}");

    // Fix: annotate the variable (`let guess: Vec<u64>`) or use the
    // "turbofish" syntax: `.collect::<Vec<u64>>()` (or `Vec<_>` to let the
    // element type be inferred).

    // Collecting Results: Vec<Result<T, E>> can collect into
    // Result<Vec<T>, E>. You get the first error, or all the values.
    let ok: Result<Vec<i64>, _> = ["100", "101"].iter().map(|s| s.parse::<i64>()).collect();
    let bad: Result<Vec<i64>, _> = ["100", "x1"].iter().map(|s| s.parse::<i64>()).collect();
    println!("  parsed ok={ok:?} bad={bad:?}");

    // iter_mut to change in place, into_iter to take elements out (no clones):
    let mut book = book;
    book.iter_mut()
        .filter(|o| o.side == Side::Sell)
        .for_each(|o| o.price -= 1);
    let client_names: Vec<String> = book.into_iter().map(|o| o.client).collect(); // moves the Strings
    println!("  took client names: {client_names:?}");
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. HashMap<K, V>
// ─────────────────────────────────────────────────────────────────────────────

fn section_hashmap() {
    println!("\n== HashMap ==");

    let mut by_id: HashMap<u64, Order> = HashMap::new();
    for o in sample_orders() {
        by_id.insert(o.id, o); // the map takes ownership of each order
    }

    // insert returns the OLD value if the key was already there:
    let replaced = by_id.insert(1, order(1, Side::Buy, 99, 5, "alice"));
    println!("  replaced old #1 @ {:?}", replaced.map(|o| o.price));

    // get -> Option<&V>, get_mut -> Option<&mut V>, remove -> Option<V>
    if let Some(o) = by_id.get(&2) {
        println!("  #2 belongs to {}", o.client);
    }
    if let Some(o) = by_id.get_mut(&2) {
        o.qty = 0; // amend in place
    }
    let cancelled = by_id.remove(&3); // cancel: you get the order back
    println!(
        "  cancelled {:?}, contains 3? {}",
        cancelled.map(|o| o.id),
        by_id.contains_key(&3)
    );

    // Iteration order is RANDOM, and can change from run to run. Never
    // depend on it. Sort the keys, or use a BTreeMap, for stable output.
    let mut keys: Vec<u64> = by_id.keys().copied().collect();
    keys.sort();
    println!("  keys sorted: {keys:?}");

    // `map[&k]` reads, and PANICS if the key is missing. Prefer `.get`.
    // You can't assign through it either:
    // ❌ UNCOMMENT -> error[E0594]: cannot assign to data in an index of `HashMap<u64, Order>`
    // by_id[&1] = order(1, Side::Buy, 98, 1, "alice");

    // A reference you got from the map keeps the whole map borrowed:
    // ❌ UNCOMMENT -> error[E0502]: cannot borrow `by_id` as mutable because it is also borrowed as immutable
    // let first = by_id.get(&1).unwrap();
    // by_id.remove(&2);
    // println!("{}", first.id);

    // THE ENTRY API: "find this key, and insert it if it's missing", with a
    // single lookup. In TS you'd write map.set(k, (map.get(k) ?? 0) + q),
    // which looks the key up twice.
    let fills = [("alice", 5i64), ("bob", -3), ("alice", -2), ("carol", 7)];
    let mut position: HashMap<&str, i64> = HashMap::new();
    for (client, qty) in fills {
        *position.entry(client).or_insert(0) += qty;
    }
    // or_default uses Default::default() (0 for numbers, empty for String/Vec):
    let mut count_by_side: HashMap<Side, u32> = HashMap::new();
    for o in sample_orders() {
        *count_by_side.entry(o.side).or_default() += 1;
    }
    // A HashMap can be turned into a sorted BTreeMap for printing:
    let sorted: BTreeMap<&str, i64> = position.into_iter().collect();
    println!("  positions {sorted:?}, buys={}", count_by_side[&Side::Buy]);
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. BTreeMap<K, V>: a sorted map, the backbone of an order book
// ─────────────────────────────────────────────────────────────────────────────
//
// A BTreeMap keeps its keys in sorted order. With PRICE as the key:
//   asks: the lowest price is the best ask  -> the FIRST entry
//   bids: the highest price is the best bid -> the LAST entry
// Insert, remove, lookup and finding the best price are all O(log n).
// A HashMap can't do this: it has no order, so "best price" would mean
// scanning every level.

fn section_btreemap() {
    println!("\n== BTreeMap ==");

    let mut asks: BTreeMap<i64, u64> = BTreeMap::new(); // price -> total qty at that price
    for (px, q) in [(102, 5), (101, 3), (105, 1), (101, 2)] {
        *asks.entry(px).or_insert(0) += q;
    }
    println!("  asks {asks:?}"); // always printed in key order
    println!("  best ask = {:?}", asks.first_key_value());

    let bids: BTreeMap<i64, u64> = [(99, 4), (100, 2), (97, 9)].into_iter().collect();
    println!("  best bid = {:?}", bids.last_key_value());

    // Walking each side from the best price outwards:
    print!("  asks best-first:");
    for (px, q) in &asks {
        print!(" {q}@{px}");
    }
    print!("\n  bids best-first:");
    for (px, q) in bids.iter().rev() {
        print!(" {q}@{px}");
    }
    println!();

    // range: every ask at or below 102, i.e. what a buy limit at 102 can trade with.
    let reachable: u64 = asks.range(..=102).map(|(_, q)| q).sum();
    println!("  qty a buy @102 could take: {reachable}");

    // pop_first / pop_last take the best level out. remove deletes one level.
    let best = asks.pop_first();
    asks.remove(&105);
    println!("  popped {best:?}, left {asks:?}");
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. VecDeque<T>: FIFO queue at one price level
// ─────────────────────────────────────────────────────────────────────────────
//
// At one price, orders fill in ARRIVAL order (time priority, first in first
// out). With a Vec, taking the front with remove(0) is O(n) because every
// other element shifts left. VecDeque is a ring buffer: pushing and popping
// at either end is O(1).

fn section_vecdeque() {
    println!("\n== VecDeque ==");

    let mut level: VecDeque<Order> = VecDeque::new();
    level.push_back(order(1, Side::Sell, 101, 3, "alice"));
    level.push_back(order(2, Side::Sell, 101, 5, "bob"));
    level.push_back(order(3, Side::Sell, 101, 2, "carol"));

    // An incoming buy for 4 takes alice's 3, then 1 from bob.
    let mut want = 4;
    while want > 0 {
        let Some(front) = level.front_mut() else {
            break;
        }; // let-else: bind or bail out
        let take = want.min(front.qty);
        front.qty -= take;
        want -= take;
        println!("  filled {take} against #{}", front.id);
        if front.qty == 0 {
            level.pop_front(); // fully filled makers leave the queue
        }
    }
    println!(
        "  queue now {:?}",
        level.iter().map(|o| (o.id, o.qty)).collect::<Vec<_>>()
    );

    // Cancelling from the middle: find it, then remove it. That's O(n), but
    // a single price level is usually short.
    if let Some(i) = level.iter().position(|o| o.id == 3) {
        let gone = level.remove(i);
        println!("  cancelled {:?}", gone.map(|o| o.id));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Sets
// ─────────────────────────────────────────────────────────────────────────────

fn section_sets() {
    println!("\n== sets ==");
    let book = sample_orders();

    let clients: HashSet<&str> = book.iter().map(|o| o.client.as_str()).collect();
    println!(
        "  {} unique clients, alice active? {}",
        clients.len(),
        clients.contains("alice")
    );
    let sorted: BTreeSet<&str> = clients.iter().copied().collect();
    println!("  sorted: {sorted:?}");

    // insert returns false if the value was already there. That makes
    // "deduplicate but keep first-seen order" a short loop:
    let mut seen = HashSet::new();
    let mut order_of_arrival = Vec::new();
    for o in &book {
        if seen.insert(o.client.as_str()) {
            order_of_arrival.push(o.client.as_str());
        }
    }
    println!("  first seen: {order_of_arrival:?}");

    let allowed: BTreeSet<&str> = ["alice", "bob", "eve"].into();
    let blocked: Vec<&&str> = sorted.difference(&allowed).collect();
    println!("  not on the allow list: {blocked:?}");
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Putting it together: an order book
// ─────────────────────────────────────────────────────────────────────────────
//
//   bids: BTreeMap<price, VecDeque<Order>>   best = last key
//   asks: BTreeMap<price, VecDeque<Order>>   best = first key
//   index: HashMap<id, (side, price)>        cancel finds the level without scanning
//
// Each of these choices is something you'll benchmark and change later
// (phase 12). It's a good, simple starting point.

#[derive(Debug, Default)]
struct Book {
    bids: BTreeMap<i64, VecDeque<Order>>,
    asks: BTreeMap<i64, VecDeque<Order>>,
    index: HashMap<u64, (Side, i64)>,
}

/// (price, total qty) for each level, best first.
type Levels = Vec<(i64, u64)>;

/// One trade against a resting (maker) order.
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
struct Fill {
    maker_id: u64,
    price: i64,
    qty: u64,
}

impl Book {
    fn add(&mut self, o: Order) {
        self.index.insert(o.id, (o.side, o.price));
        let levels = match o.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        // or_default() creates an empty VecDeque the first time a price is seen.
        levels.entry(o.price).or_default().push_back(o);
    }

    fn best_bid(&self) -> Option<i64> {
        self.bids.keys().next_back().copied() // last key
    }

    fn best_ask(&self) -> Option<i64> {
        self.asks.keys().next().copied() // first key
    }

    fn cancel(&mut self, id: u64) -> Option<Order> {
        let (side, price) = self.index.remove(&id)?;
        let levels = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        let level = levels.get_mut(&price)?;
        let pos = level.iter().position(|o| o.id == id)?;
        let o = level.remove(pos)?;
        if level.is_empty() {
            // `level` borrows `levels`, but this is its last use, so the
            // borrow is over and we can mutate `levels` (phase 4, NLL).
            levels.remove(&price);
        }
        Some(o)
    }
}

fn section_order_book() {
    println!("\n== order book ==");
    let mut book = Book::default();
    for o in sample_orders() {
        book.add(o);
    }
    println!(
        "  best bid {:?}, best ask {:?}",
        book.best_bid(),
        book.best_ask()
    );
    println!("  bid levels: {:?}", book.bids.keys().collect::<Vec<_>>());

    let gone = book.cancel(3).map(|o| o.id);
    println!(
        "  cancel #3 -> {gone:?}; level 101 still has #4: {:?}",
        book.bids
            .get(&101)
            .map(|l| l.iter().map(|o| o.id).collect::<Vec<_>>())
    );
    let gone = book.cancel(4).map(|o| o.id);
    println!(
        "  cancel #4 -> {gone:?}; best bid now {:?}",
        book.best_bid()
    );
    println!("  cancel #4 again -> {:?}", book.cancel(4).map(|o| o.id));
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. PUZZLES. Each block has a compile error. Uncomment one, read the error,
//    and fix it.
// ─────────────────────────────────────────────────────────────────────────────
fn puzzles() {
    println!("\n== puzzles (uncomment in source) ==");

    // PUZZLE 1: total buy qty. (Hint: read what type `sum` is asked to produce.)
    // let book = sample_orders();
    // let total = book.iter().filter(|o| o.side == Side::Buy).map(|o| o.qty).sum();
    // println!("{total}");

    // PUZZLE 2: count orders per client, then print them.
    // let mut counts: HashMap<String, u32> = HashMap::new();
    // for o in sample_orders() { counts.entry(o.client) += 1; }
    // println!("{counts:?}");

    // PUZZLE 3: remove fully filled orders while scanning the level.
    // let mut level: VecDeque<Order> = VecDeque::from(sample_orders());
    // for (i, o) in level.iter().enumerate() { if o.qty == 0 { level.remove(i); } }

    // PUZZLE 4: keep the best ask price, then remove its level.
    // let mut asks: BTreeMap<i64, u64> = [(101, 3), (102, 1)].into_iter().collect();
    // let (best_px, _) = asks.first_key_value().unwrap();
    // asks.remove(best_px);
    // println!("removed {best_px}");
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES: `cargo test --bin p06_collections`
// Try iterator chains where they fit. A plain `for` loop is fine when it's
// clearer.
// ═════════════════════════════════════════════════════════════════════════════

/// Total open qty per client. The keys borrow the clients' names from
/// `orders` (no new Strings).
/// Hint: entry API.
#[allow(dead_code)]
fn open_qty_by_client(orders: &[Order]) -> HashMap<&str, u64> {
    let _ = orders;
    let mut order_counts: HashMap<&str, u64> = HashMap::new();

    for order in orders {
        *order_counts.entry(&order.client).or_insert(0) += order.qty;
    }

    order_counts
}

/// The ids of the BUY orders only, in price-time priority: highest price
/// first, then lowest id first. Ignore sells.
#[allow(dead_code)]
fn bid_priority(orders: &[Order]) -> Vec<u64> {
    let mut buy_orders = orders
        .iter()
        .filter(|o| o.side == Side::Buy)
        .collect::<Vec<_>>();
    buy_orders.sort_by(|a, b| b.price.cmp(&a.price).then(a.id.cmp(&b.id)));

    buy_orders.into_iter().map(|o| o.id).collect()
    // let buy_orders = orders.sort_by(|o| o);.iter().filter(|o| o.side == Side::Buy);
}

/// Each client name once, in the order they first appear.
#[allow(dead_code)]
fn first_seen_clients(orders: &[Order]) -> Vec<&str> {
    let mut names_map: Vec<&str> = Vec::new();
    let mut seen_names: HashSet<&str> = HashSet::new();

    for order in orders {
        if seen_names.insert(&order.client.as_str()) {
            names_map.push(&order.client);
        }
    }

    names_map.into_iter().collect()
}

impl Book {
    /// Up to `n` levels per side, as (price, total qty at that price).
    /// Bids best-first (high to low), asks best-first (low to high).
    #[allow(dead_code)]
    fn depth(&self, n: usize) -> (Levels, Levels) {
        let bids = &self.bids;
        let asks = &self.asks;
        let mut bid_level = Levels::new();

        for (bid_price, bid_queue) in bids.iter().rev().take(n) {
            bid_level.push((*bid_price, bid_queue.iter().map(|o| o.qty).sum::<u64>()));
        }

        let mut ask_levels = Levels::new();
        for (ask_price, ask_queue) in asks.iter().take(n) {
            ask_levels.push((*ask_price, ask_queue.iter().map(|o| o.qty).sum::<u64>()));
        }

        (bid_level, ask_levels)
    }

    /// A market BUY for `qty`. Walk the asks from the lowest price; at each
    /// price fill makers in arrival order (FIFO). Return the fills in the
    /// order they happen. Afterwards:
    ///   - partly filled makers stay at the front with less qty
    ///   - fully filled makers are gone, from their level AND from `index`
    ///   - empty levels are removed
    ///   - if the book runs out, return what was filled
    ///
    /// Hint: `while let Some(mut entry) = self.asks.first_entry()` hands you
    /// the best level; `entry.get_mut()` is its VecDeque, and
    /// `entry.remove()` deletes it.
    #[allow(dead_code)]
    fn market_buy(&mut self, qty: u64) -> Vec<Fill> {
        let mut fills: Vec<Fill> = Vec::new();
        let mut remaining_qty = qty;

        while remaining_qty > 0 {
            let Some(mut entry) = self.asks.first_entry() else {
                break;
            };

            while remaining_qty > 0 {
                let queue = entry.get_mut();
                let first_order = queue.get_mut(0);
                let Some(front) = first_order else {
                    break;
                };
                let fill_qty = remaining_qty.min(front.qty);

                front.qty -= fill_qty;
                remaining_qty -= fill_qty;

                fills.push(Fill {
                    maker_id: front.id,
                    price: front.price,
                    qty: fill_qty,
                });

                if front.qty == 0 {
                    let filled_order = queue.pop_front().unwrap();
                    self.index.remove(&filled_order.id);
                }

                if entry.get().is_empty() {
                    entry.remove();
                    break;
                }
            }
        }

        fills
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_open_qty_by_client() {
        let orders = sample_orders();
        let m = open_qty_by_client(&orders);
        assert_eq!(m.len(), 4);
        assert_eq!(m["alice"], 9);
        assert_eq!(m["dave"], 7);
        assert!(open_qty_by_client(&[]).is_empty());
    }

    #[test]
    fn t_bid_priority() {
        assert_eq!(bid_priority(&sample_orders()), vec![3, 4, 1]);
        let mut v = sample_orders();
        v.reverse();
        assert_eq!(bid_priority(&v), vec![3, 4, 1]); // order in the input doesn't matter
    }

    #[test]
    fn t_first_seen_clients() {
        let mut v = sample_orders();
        v.push(order(6, Side::Buy, 1, 1, "bob"));
        assert_eq!(
            first_seen_clients(&v),
            vec!["alice", "bob", "carol", "dave"]
        );
    }

    fn test_book() -> Book {
        let mut b = Book::default();
        for o in [
            order(10, Side::Buy, 100, 5, "a"),
            order(11, Side::Buy, 100, 2, "b"),
            order(12, Side::Buy, 99, 1, "c"),
            order(13, Side::Buy, 98, 4, "d"),
            order(1, Side::Sell, 101, 3, "e"),
            order(2, Side::Sell, 101, 2, "f"),
            order(3, Side::Sell, 102, 5, "g"),
        ] {
            b.add(o);
        }
        b
    }

    #[test]
    fn t_depth() {
        let b = test_book();
        let (bids, asks) = b.depth(2);
        assert_eq!(bids, vec![(100, 7), (99, 1)]);
        assert_eq!(asks, vec![(101, 5), (102, 5)]);
        let (bids, asks) = b.depth(10);
        assert_eq!(bids.len(), 3);
        assert_eq!(asks.len(), 2);
    }

    #[test]
    fn t_market_buy() {
        let mut b = test_book();
        let fills = b.market_buy(4);
        assert_eq!(
            fills,
            vec![
                Fill {
                    maker_id: 1,
                    price: 101,
                    qty: 3
                },
                Fill {
                    maker_id: 2,
                    price: 101,
                    qty: 1
                }
            ]
        );
        assert_eq!(b.best_ask(), Some(101)); // #2 still rests with 1 left
        assert_eq!(b.asks[&101].front().map(|o| (o.id, o.qty)), Some((2, 1)));
        assert!(!b.index.contains_key(&1)); // fully filled makers leave the index

        let fills = b.market_buy(100); // more than the book has
        assert_eq!(
            fills,
            vec![
                Fill {
                    maker_id: 2,
                    price: 101,
                    qty: 1
                },
                Fill {
                    maker_id: 3,
                    price: 102,
                    qty: 5
                }
            ]
        );
        assert!(b.asks.is_empty());
        assert_eq!(b.best_ask(), None);
        assert!(b.cancel(3).is_none());
        assert_eq!(b.best_bid(), Some(100)); // bids untouched
        assert!(b.market_buy(1).is_empty());
    }
}
