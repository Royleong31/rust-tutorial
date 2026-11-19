//! PHASE 5 — Lifetimes
//!
//!   cargo run  --bin p05_lifetimes
//!   cargo test --bin p05_lifetimes
//!
//! A LIFETIME is the stretch of code during which a reference is valid.
//! Every reference has one. Usually the compiler works it out and you never
//! see it. You write one (`'a`) only when the compiler can't tell how the
//! references in a signature or a struct relate to each other.
//!
//! THE KEY IDEA: a lifetime annotation never changes how long anything
//! lives. Owners decide that (phase 3). An annotation is a PROMISE in a
//! signature, such as "the returned reference points into this argument".
//! The compiler checks the promise inside the function and at every call.
//!
//! THE MODEL, in one line:
//!
//!     a reference's lifetime  ≤  the lifetime of the value it points at
//!
//! Everything in this file is the compiler checking that inequality.
//!
//! In TS, holding a reference keeps an object alive (the GC sees it). In
//! Rust the owner frees the value when its scope ends, and the borrow
//! checker rejects any reference that would still be used after that.
//!
//! Reading a lifetime error, in three steps:
//!   1. Which value is being borrowed? ("`buf` does not live long enough")
//!   2. Where does it die? (rustc marks the `}` with "dropped here")
//!   3. Where is the reference still used after that? ("borrow later used here")
//!
//! The fix is almost always one of: keep the owner alive longer, stop using
//! the reference sooner, or return/store an OWNED value instead.

fn main() {
    section_dangling();
    section_function_lifetimes();
    section_elision();
    section_struct_lifetimes();
    section_multiple_lifetimes();
    section_static();
    section_owned_vs_borrowed();
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
// 1. Dangling references: the bug lifetimes prevent
// ─────────────────────────────────────────────────────────────────────────────

fn section_dangling() {
    println!("\n== dangling references ==");

    let r;
    {
        let px = 100;
        r = &px;
        println!("  inside the scope r = {r}"); // fine: px is still alive
    } // px is dropped here, so r must not be used below this line

    // ❌ UNCOMMENT -> error[E0597]: `client` does not live long enough
    //   rustc marks three places: the borrow, "dropped here while still
    //   borrowed", and "borrow later used here".
    // let name;
    // {
    //     let client: String = String::from("alice");
    //     name = &client;
    // }
    // println!("{name}");

    // A function can't return a reference to its own local variable either.
    // The local is freed when the function returns. PUZZLE 3 at the bottom
    // shows the error (E0515).
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Functions that return references
// ─────────────────────────────────────────────────────────────────────────────
//
// Two reference inputs and one reference output. Which input does the output
// point into? The body picks one at runtime, so the compiler can't tell from
// the signature, and it makes you say.
//
// ❌ UNCOMMENT -> error[E0106]: missing lifetime specifier
// fn better_bid_bad(a: &Order, b: &Order) -> &Order { if a.price >= b.price { a } else { b } }

// `'a` is a generic LIFETIME parameter, declared in <> like a type
// parameter. Read the signature as: "for some region 'a where both inputs
// are valid, the output is valid for 'a too". At a call site 'a ends up as
// the SHORTER of the two borrows.
fn better_bid<'a>(a: &'a Order, b: &'a Order) -> &'a Order {
    if a.price >= b.price { a } else { b }
}

fn section_function_lifetimes() {
    println!("\n== functions returning references ==");

    let alice = order(1, Side::Buy, 100, 5, "alice");
    {
        let bob = order(2, Side::Buy, 101, 1, "bob");
        let best = better_bid(&alice, &bob);
        println!("  best = #{}", best.id); // fine: both are alive here
    }

    // ❌ UNCOMMENT -> error[E0597]: `bob` does not live long enough
    // let winner;
    // {
    //     let bob = order(2, Side::Buy, 101, 1, "bob");
    //     winner = better_bid(&alice, &bob);
    // }
    // println!("{}", winner.id);

    // Even if `alice` would win at runtime, this still fails. The compiler
    // checks the SIGNATURE, not the values: the signature says the result
    // could point into either argument.
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Elision: why you rarely write lifetimes
// ─────────────────────────────────────────────────────────────────────────────
//
// The compiler fills in lifetimes using three rules:
//   1. Each reference PARAMETER gets its own lifetime.
//   2. If there's exactly ONE input lifetime, every output reference gets it.
//   3. If one input is `&self` or `&mut self`, every output reference gets
//      self's lifetime.
// If the output is still unknown after that, you get E0106 and must write it.

// Rule 2. Short for: fn symbol_base<'a>(s: &'a str) -> &'a str
fn symbol_base(s: &str) -> &str {
    s.split('-').next().unwrap_or(s)
}

// Two inputs, but the result only ever comes from `orders`. Give `client`
// no named lifetime (it gets its own by rule 1), so the caller can drop the
// search string while still using the result.
#[allow(clippy::manual_find)] // phase 6 shows the shorter iterator version
fn find_client<'a>(orders: &'a [Order], client: &str) -> Option<&'a Order> {
    for o in orders {
        if o.client == client {
            return Some(o);
        }
    }
    None
}

// The same body with ONE lifetime on everything. It compiles, but it
// promises more than it needs: callers must now keep `client` alive as long
// as they use the result.
#[allow(dead_code)]
fn find_client_strict<'a>(orders: &'a [Order], client: &'a str) -> Option<&'a Order> {
    find_client(orders, client)
}

fn section_elision() {
    println!("\n== elision ==");
    println!("  symbol_base(\"BTC-USD\") = {}", symbol_base("BTC-USD"));

    let orders = vec![
        order(1, Side::Buy, 100, 5, "alice"),
        order(2, Side::Sell, 101, 3, "bob"),
    ];

    let found;
    {
        let query = String::from("bob"); // e.g. read from a request, short-lived
        found = find_client(&orders, &query);
    } // query dropped here; found borrows only from `orders`, so that's fine
    println!("  found = {:?}", found.map(|o| o.id));

    // ❌ UNCOMMENT -> error[E0597]: `query2` does not live long enough
    // let found2;
    // {
    //     let query2 = String::from("bob");
    //     found2 = find_client_strict(&orders, &query2);
    // }
    // println!("{:?}", found2.map(|o| o.id));

    // Lesson: tie the output only to the inputs it really comes from.
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Structs that hold references (zero-copy parsing)
// ─────────────────────────────────────────────────────────────────────────────
//
// A struct holding a reference needs a lifetime parameter. It says the
// struct can't outlive the data it points into.
//
// ❌ UNCOMMENT -> error[E0106]: missing lifetime specifier
// struct BadView { client: &str }
//
// The real use is ZERO-COPY PARSING. A FIX message (the protocol most
// exchanges speak) arrives as one buffer of tag=value pairs. A parsed view
// can point into that buffer instead of allocating a String for every
// field, which matters when you parse millions of messages.
//
//   8=FIX.4.4|35=D|49=ALICE|55=BTC-USD|54=1|44=6500025|38=5|
//
//   35=D new order   49 sender   55 symbol   54 side (1=buy, 2=sell)
//   44 price (ticks) 38 qty
// (Real FIX separates fields with the byte \x01. We use '|' so you can read it.)

#[derive(Debug)]
struct FixMsg<'a> {
    fields: Vec<(u32, &'a str)>, // each value is a window into the original buffer
}

impl<'a> FixMsg<'a> {
    // `raw: &'a str` and `FixMsg<'a>`: the message borrows from `raw`.
    fn parse(raw: &'a str) -> Option<FixMsg<'a>> {
        let mut fields = Vec::new();
        for pair in raw.split('|') {
            if pair.is_empty() {
                continue;
            }
            // `?` on an Option returns None from the function early if the
            // value is None (more in phase 7).
            let (tag, value) = pair.split_once('=')?;
            fields.push((tag.parse().ok()?, value));
        }
        Some(FixMsg { fields })
    }

    // Returns `&'a str`: the value points into the BUFFER, not into this
    // FixMsg. So it stays usable after the FixMsg is gone.
    fn get(&self, tag: u32) -> Option<&'a str> {
        for &(t, v) in &self.fields {
            if t == tag {
                return Some(v);
            }
        }
        None
    }

    // The same thing with the lifetime left out. Elision rule 3 ties the
    // output to `&self`, which is a shorter, stricter promise than needed.
    fn get_tied(&self, tag: u32) -> Option<&str> {
        self.get(tag)
    }
}

fn section_struct_lifetimes() {
    println!("\n== structs with references ==");

    let raw = String::from("8=FIX.4.4|35=D|49=ALICE|55=BTC-USD|54=1|44=6500025|38=5|");
    let msg = FixMsg::parse(&raw).expect("bad FIX");
    println!(
        "  {} fields, tied lookup: {:?}",
        msg.fields.len(),
        msg.get_tied(35)
    );

    let sym = msg.get(55).unwrap();
    let inside = raw.as_bytes().as_ptr_range().contains(&sym.as_ptr());
    println!("  symbol={sym}, points into raw: {inside}"); // no copy was made

    drop(msg); // the parsed view is gone...
    println!("  after drop(msg) the symbol still works: {sym}"); // ...because sym borrows raw

    // With the tied version the same thing fails, because the result
    // borrows `msg` itself:
    // ❌ UNCOMMENT -> error[E0505]: cannot move out of `msg2` because it is borrowed
    // let msg2 = FixMsg::parse(&raw).unwrap();
    // let tied = msg2.get_tied(55).unwrap();
    // drop(msg2);
    // println!("{tied}");

    // And no view may outlive the buffer:
    // ❌ UNCOMMENT -> error[E0597]: `buf` does not live long enough
    // let view;
    // {
    //     let buf = String::from("35=D|55=ETH-USD|");
    //     view = FixMsg::parse(&buf).unwrap();
    // }
    // println!("{:?}", view.get(55));
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Multiple named lifetimes
// ─────────────────────────────────────────────────────────────────────────────
//
// find_client already had two lifetimes: 'a for `orders`, plus an unnamed one
// for `client`. Written out in full, its signature is
//
//     fn find_client<'a, 'b>(orders: &'a [Order], client: &'b str) -> Option<&'a Order>
//
// You have to NAME both when a struct holds two references that can expire
// at different times. This query borrows the book AND a search string:

struct ClientQuery<'book, 'q> {
    orders: &'book [Order],
    client: &'q str,
}

impl<'book, 'q> ClientQuery<'book, 'q> {
    // The result points into the BOOK, so it gets 'book. It doesn't depend on
    // 'q, so it can outlive both the search string and the query itself.
    fn first(&self) -> Option<&'book Order> {
        find_client(self.orders, self.client)
    }
}

// The same struct with ONE lifetime for both fields:
#[allow(dead_code)]
struct ClientQueryOne<'a> {
    orders: &'a [Order],
    client: &'a str,
}

#[allow(dead_code)]
impl<'a> ClientQueryOne<'a> {
    fn first(&self) -> Option<&'a Order> {
        find_client(self.orders, self.client)
    }
}

fn section_multiple_lifetimes() {
    println!("\n== multiple lifetimes ==");
    let orders = vec![
        order(1, Side::Buy, 100, 5, "alice"),
        order(2, Side::Sell, 101, 3, "bob"),
    ];

    let hit;
    {
        let needle = String::from("alice"); // short-lived, like a request parameter
        let q = ClientQuery {
            orders: &orders,
            client: &needle,
        };
        hit = q.first();
    } // needle and q are gone, but hit only borrows `orders`
    println!("  hit = {:?}", hit.map(|o| o.id));

    // With one shared lifetime, 'a has to fit inside BOTH borrows, so it
    // shrinks to the shorter one (needle's) and the result can't outlive it:
    // ❌ UNCOMMENT -> error[E0597]: `needle2` does not live long enough
    // let hit2;
    // {
    //     let needle2 = String::from("alice");
    //     let q = ClientQueryOne { orders: &orders, client: &needle2 };
    //     hit2 = q.first();
    // }
    // println!("{:?}", hit2.map(|o| o.id));

    // Most of the time one lifetime is enough. Reach for a second when a
    // result should outlive one of the inputs.
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. 'static
// ─────────────────────────────────────────────────────────────────────────────
//
// `'static` is the lifetime of the whole program run. It shows up in two
// different ways:
//
//   &'static str: a reference that's valid forever. String literals are
//     stored in the compiled binary, so they never go away. Phase 1's
//     hold_label returned these.
//
//   T: 'static (a BOUND on a generic type): T holds no borrowed data that
//     could expire. Owned types like String, Vec<Order> and i64 all satisfy
//     it. It does NOT mean the value lives forever, only that it's ALLOWED
//     to. Threads require it (phase 10): a spawned thread can outlive the
//     function that started it, so it can't borrow that function's locals.

fn side_name(s: Side) -> &'static str {
    match s {
        Side::Buy => "BUY",
        Side::Sell => "SELL",
    }
}

// Stands in for "hand this to a background thread".
fn send_to_background<T: std::fmt::Debug + 'static>(job: T) {
    println!("  background got {job:?}");
}

fn section_static() {
    println!("\n== 'static ==");

    let name: &'static str = side_name(Side::Buy);
    println!("  side name = {name}");

    send_to_background(String::from("owned")); // owns its data: fine
    send_to_background(42); // fine
    send_to_background("literal"); // &'static str: fine

    let local = String::from("borrowed");
    // ❌ UNCOMMENT -> error[E0597]: `local` does not live long enough
    //   ("argument requires that `local` is borrowed for `'static`").
    //   You'll also get E0505 for the line below it, which moves `local`
    //   while it's still borrowed. Fix the first error and both go away.
    // send_to_background(&local);

    // Fix: give it ownership, which is exactly what you do with threads.
    send_to_background(local);
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Design rule: when to own, when to borrow
// ─────────────────────────────────────────────────────────────────────────────
//
//   LONG-LIVED state (resting orders, the book, accounts): OWN the data
//     (String, Vec). No lifetime parameters, so it can go anywhere: in a
//     HashMap, across threads, returned from functions.
//
//   SHORT-LIVED views (a parser over an input buffer, a snapshot for
//     printing, an iterator): BORROW, with a lifetime.
//
// A common beginner mistake is putting `&'a str` inside Order "to avoid
// allocations". The lifetime then spreads to Book<'a>, Engine<'a>, ... and
// nothing can outlive the network buffer. Convert to owned data ONCE, at the
// boundary, for the data you keep:

#[derive(Debug)]
struct NewOrder {
    client: String,
    symbol: String,
    side: Side,
    price: i64,
    qty: u64,
}

// `impl FixMsg<'_>`: '_ means "some lifetime, I don't need to name it here".
impl FixMsg<'_> {
    fn to_new_order(&self) -> Option<NewOrder> {
        let side = match self.get(54)? {
            "1" => Side::Buy,
            "2" => Side::Sell,
            _ => return None,
        };
        Some(NewOrder {
            client: self.get(49)?.to_string(), // allocate once, for data we keep
            symbol: self.get(55)?.to_string(),
            side,
            price: self.get(44)?.parse().ok()?,
            qty: self.get(38)?.parse().ok()?,
        })
    }
}

fn section_owned_vs_borrowed() {
    println!("\n== own long-lived data ==");

    let accepted: Option<NewOrder> = {
        let buf = String::from("35=D|49=BOB|55=ETH-USD|54=2|44=350010|38=3|"); // a network buffer
        let msg = FixMsg::parse(&buf);
        msg.and_then(|m| m.to_new_order())
    }; // buf and msg are dropped here; `accepted` owns its own copies
    println!("  {accepted:?}");
    if let Some(o) = &accepted {
        println!(
            "  {} wants to {} {} {} @ {}",
            o.client,
            side_name(o.side),
            o.qty,
            o.symbol,
            o.price
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. PUZZLES. Each block has a compile error. Uncomment one, read the error,
//    and fix it.
// ─────────────────────────────────────────────────────────────────────────────
fn puzzles() {
    println!("\n== puzzles (uncomment in source) ==");

    // PUZZLE 1: return whichever order is bigger. Add the missing lifetime.
    // fn bigger(a: &Order, b: &Order) -> &Order { if a.qty >= b.qty { a } else { b } }
    // let (x, y) = (order(1, Side::Buy, 1, 5, "a"), order(2, Side::Buy, 1, 9, "b"));
    // println!("{}", bigger(&x, &y).id);

    // PUZZLE 2: a top-of-book snapshot that borrows the best order.
    // struct Snapshot { best: &Order }
    // let o = order(1, Side::Buy, 100, 1, "a");
    // println!("{}", Snapshot { best: &o }.best.id);

    // PUZZLE 3: build a symbol from a base asset. No lifetime can fix this
    //   one. Why not, and what's the real fix?
    // fn make_symbol(base: &str) -> &str { let s = format!("{base}-USD"); &s }
    // println!("{}", make_symbol("BTC"));

    // PUZZLE 4: look up a FIX field. The needle is temporary. Fix the
    //   SIGNATURE, not the caller.
    // fn field<'a>(raw: &'a str, prefix: &'a str) -> Option<&'a str> { raw.split('|').find(|f| f.starts_with(prefix)) }
    // let raw = String::from("55=BTC-USD|54=1|");
    // let found;
    // { let needle = String::from("55="); found = field(&raw, &needle); }
    // println!("{found:?}");
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES: `cargo test --bin p05_lifetimes`
// No .clone(), no .to_string(). Every result borrows from the input.
// ═════════════════════════════════════════════════════════════════════════════

/// Find the value of `tag` directly in a raw FIX string, without building a
/// FixMsg. Return None if the tag isn't there. Watch out: looking for tag 5
/// must not match "55=...".
/// Hint: split on '|', then split_once('=') each piece.
#[allow(dead_code)]
fn tag_value(raw: &str, tag: u32) -> Option<&str> {
    let msgs = raw.split('|');

    for msg in msgs {
        let Some((t, value)) = msg.split_once('=') else {
            continue;
        };

        let Ok(parsed_value) = t.parse::<u32>() else {
            continue;
        };

        if parsed_value == tag {
            return Some(value);
        }
    }

    None
}

/// A symbol split into its two assets, both pointing into the input.
#[derive(Debug, PartialEq)]
struct SymbolParts<'a> {
    base: &'a str,
    quote: &'a str,
}

/// "BTC-USD" -> SymbolParts { base: "BTC", quote: "USD" }. None if there's no
/// '-'. (`'_` in the return type means "borrows from the one input".)
#[allow(dead_code)]
fn split_symbol(s: &str) -> Option<SymbolParts<'_>> {
    let Some((base, quote)) = s.split_once('-') else {
        return None;
    };

    return Some(SymbolParts { base, quote });
}

/// The client name of the order with the LONGEST client name (the first one
/// on ties), or None if `orders` is empty. Return a &str pointing into the
/// Order's own String.
#[allow(dead_code)]
fn longest_client(orders: &[Order]) -> Option<&str> {
    if orders.is_empty() {
        return None;
    }

    let mut current_longest = &orders[0].client;

    for order in orders {
        if order.client.len() > current_longest.len() {
            current_longest = &order.client;
        }
    }

    Some(current_longest)
}

impl<'a> FixMsg<'a> {
    /// Every value for `tag`, in order. FIX allows repeated tags in
    /// "repeating groups". The test drops the FixMsg and keeps using the
    /// results. Look at the return type to see why that's allowed.
    #[allow(dead_code)]
    fn get_all(&self, tag: u32) -> Vec<&'a str> {
        let mut values: Vec<&'a str> = Vec::new();

        for (t, value) in &self.fields {
            if *t == tag {
                values.push(value);
            }
        }

        values
    }
}

// BONUS (write the signature yourself):
// Write `fn smaller_qty(...)`, which takes two &Order and returns the one
// with the smaller qty (the first one on ties). You'll need a lifetime.
// Then delete the #[ignore] line on the last test and uncomment its body.

fn smaller_qty<'a>(o1: &'a Order, o2: &'a Order) -> &'a Order {
    if o1.qty <= o2.qty {
        return o1;
    }

    return o2;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_tag_value() {
        let raw = String::from("8=FIX.4.4|35=D|5=x|55=BTC-USD|38=5|");
        assert_eq!(tag_value(&raw, 55), Some("BTC-USD"));
        assert_eq!(tag_value(&raw, 5), Some("x"));
        assert_eq!(tag_value(&raw, 44), None);
        let v = tag_value(&raw, 35).unwrap();
        assert!(raw.as_bytes().as_ptr_range().contains(&v.as_ptr())); // borrowed, not copied
    }

    #[test]
    fn t_split_symbol() {
        let s = String::from("ETH-USDT");
        let parts = split_symbol(&s).unwrap();
        assert_eq!(
            parts,
            SymbolParts {
                base: "ETH",
                quote: "USDT"
            }
        );
        assert_eq!(parts.base.as_ptr(), s.as_ptr());
        assert_eq!(split_symbol("BTCUSD"), None);
    }

    #[test]
    fn t_longest_client() {
        let orders = vec![
            order(1, Side::Buy, 1, 1, "bob"),
            order(2, Side::Buy, 1, 1, "carol"),
            order(3, Side::Buy, 1, 1, "alice"),
        ];
        let name = longest_client(&orders).unwrap();
        assert_eq!(name, "carol");
        assert_eq!(name.as_ptr(), orders[1].client.as_ptr()); // points into order #2
        assert_eq!(longest_client(&[]), None);
    }

    #[test]
    fn t_get_all() {
        let raw = String::from("35=D|448=ALICE|447=D|448=DESK7|");
        let msg = FixMsg::parse(&raw).unwrap();
        let parties = msg.get_all(448);
        drop(msg); // only compiles because the results borrow `raw`, not `msg`
        assert_eq!(parties, vec!["ALICE", "DESK7"]);
    }

    #[test]
    fn t_smaller_qty() {
        let a = order(1, Side::Buy, 100, 5, "a");
        let b = order(2, Side::Buy, 100, 9, "b");
        assert_eq!(smaller_qty(&a, &b).id, 1);
        assert_eq!(smaller_qty(&b, &a).id, 1);
        assert!(std::ptr::eq(smaller_qty(&a, &b), &a));
    }
}
