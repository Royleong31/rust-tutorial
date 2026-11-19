//! PHASE 1 — Language basics
//!
//!   cargo run  --bin p01_basics
//!   cargo test --bin p01_basics
//!
//! Read top to bottom. Each `section_*` function is one topic and `main` calls
//! them in order. Blocks marked `❌ UNCOMMENT` don't compile on purpose:
//! uncomment one, run `cargo build`, read the compiler error, then put it back.
//! Learning to read rustc's errors is half the skill.
//!
//! The running theme is trading: prices, quantities and fills.

use tokio::io::split;

fn main() {
    println!("price_to_ticks: {}", price_to_ticks("100.23"));
    // section_variables();
    // section_integers();
    // section_floats_and_why_prices_are_integers();
    // section_char_bool_tuples_arrays();
    // section_expressions_vs_statements();
    // section_control_flow();
    // section_basic_match();
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Variables: immutable by default, `mut`, shadowing
// ─────────────────────────────────────────────────────────────────────────────
fn section_variables() {
    println!("\n== variables ==");

    // `let` is like TS `const`: the binding can't be reassigned.
    // Rust goes further than TS, though. A TS `const` object can still be
    // mutated through the binding (obj.x = 1). A Rust binding without `mut`
    // can't be mutated at all, fields included.
    let best_bid = 10_050; // underscores are just digit separators
    println!("best_bid = {best_bid}"); // {name} captures the variable directly

    // ❌ UNCOMMENT -> error[E0384]: cannot assign twice to immutable variable
    // best_bid = 10_051;

    // `mut` opts in to mutation. Grep for `mut` and you find every place
    // where state changes, which helps a lot when reviewing trading code.
    let mut position = 0;
    position += 5;
    position -= 2;
    println!("position = {position}");

    // SHADOWING: a new `let` with the same name makes a *new* variable that
    // hides the old one. It can even have a different type. The usual use is
    // parsing: raw text -> validated value, keeping the same name.
    let qty = "250"; // &str (text)
    let qty: u32 = qty.parse().expect("not a number"); // u32 now
    println!("qty parsed = {qty}");
    // TS would need `qtyStr` and `qty`. Here the old `qty` simply can't be
    // reached any more, so you can't accidentally use the unparsed one.

    // Shadowing is NOT mutation. The inner scope's `qty` disappears at `}`.
    {
        let qty = qty * 2;
        println!("inner qty = {qty}");
    }
    println!("outer qty still = {qty}");
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Integers. This is the first real difference from JS, where every number
//    is an f64.
// ─────────────────────────────────────────────────────────────────────────────
fn section_integers() {
    println!("\n== integers ==");

    // Signed:   i8 i16 i32 i64 i128 isize
    // Unsigned: u8 u16 u32 u64 u128 usize
    // Without a hint, integer literals default to i32.
    let a: u8 = 255;
    let b: i64 = -9_000_000_000;
    let c = 42u64; // the type can go on the end of the literal

    // usize is pointer-sized (64 bits on your Mac). It's the type of every
    // index and length: vec.len() returns usize, and arr[i] needs i: usize.
    let levels = [100, 101, 102];
    let i: usize = 2;
    println!("a={a} b={b} c={c} level[{i}]={}", levels[i]);

    // ❌ UNCOMMENT -> error[E0277]: the type `[{integer}]` cannot be indexed by `i32`
    let j = 1;
    println!("{}", levels[j]);
    // Fix: levels[j as usize], or better, make j a usize to begin with.

    // OVERFLOW. In debug builds, `a + 1` here PANICS at runtime. In release
    // builds (--release) it silently wraps to 0. Don't depend on either:
    // when overflow is possible, say what should happen.
    let x: u8 = 250;
    println!("checked    250+10 = {:?}", x.checked_add(10)); // None (Option, phase 2)
    println!("saturating 250+10 = {}", x.saturating_add(10)); // 255
    println!("wrapping   250+10 = {}", x.wrapping_add(10)); // 4

    // In trading code, notional = price * qty can overflow when values are
    // large. checked_mul makes you deal with that case.
    let price_ticks: i64 = 6_500_000; // $65,000.00 in cents
    let qty: i64 = 1_000;
    println!("notional = {:?}", price_ticks.checked_mul(qty));

    // CASTS with `as` never fail, but they can silently truncate or wrap.
    let big: i64 = 300;
    println!("300 as u8 = {}", big as u8); // 44 (!)
    // Safer: u8::try_from(big), which returns a Result. That's in phase 7.
    println!("u8::try_from(300) = {:?}", u8::try_from(big));

    // Rust never converts between integer types for you:
    // ❌ UNCOMMENT -> error[E0308]: mismatched types (expected `i64`, found `u32`)
    // let qty_u32: u32 = 5;
    // let total: i64 = price_ticks + qty_u32;
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Floats, and why trading systems avoid them for prices
// ─────────────────────────────────────────────────────────────────────────────
fn section_floats_and_why_prices_are_integers() {
    println!("\n== floats ==");

    let x: f64 = 0.1 + 0.2;
    println!("0.1 + 0.2 = {x}"); // 0.30000000000000004, same as JS
    println!("0.1 + 0.2 == 0.3 ? {}", x == 0.3);

    // The usual fix is to store prices as integer TICKS (the smallest price
    // increment) and quantities as integer LOTS. Only convert to decimal at
    // the edges: parsing input and displaying output.
    //
    // BTC-USD with tick size 0.01:  $65,000.25 -> 6_500_025 ticks
    let tick_size_decimals = 2;
    let price_ticks: i64 = 6_500_025;
    let scale = 10_i64.pow(tick_size_decimals);
    println!(
        "price = {}.{:02}",
        price_ticks / scale, // integer division truncates toward zero
        price_ticks % scale
    );

    // f64 is fine for analytics like volatility, z-scores and PnL charts.
    // Keep it out of the matching engine and order state.
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. char, bool, tuples, arrays
// ─────────────────────────────────────────────────────────────────────────────
fn section_char_bool_tuples_arrays() {
    println!("\n== char / bool / tuple / array ==");

    // char is a 4-byte Unicode scalar value, not a byte and not a string.
    // Single quotes mean char, double quotes mean string.
    let side_code: char = 'B';
    let is_buy: bool = side_code == 'B';
    println!("side_code={side_code} is_buy={is_buy}");

    // TUPLES: fixed size, mixed types, elements accessed by position.
    // Similar to a TS tuple type `[number, number]`.
    let top_of_book: (i64, i64) = (10_050, 10_052); // (bid, ask)
    let (bid, ask) = top_of_book; // destructuring, same as TS
    println!(
        "bid={} ask={} spread={}",
        top_of_book.0,
        top_of_book.1,
        ask - bid
    );

    // The empty tuple `()` is called "unit". It's Rust's `void`: functions
    // without a return type actually return ().

    // ARRAYS: fixed length, and the length is PART OF THE TYPE.
    // [i64; 5] and [i64; 6] are different types. Arrays live inline (usually
    // on the stack) with no heap allocation. For growable lists use
    // Vec<T> (phase 6), which is what a JS array really corresponds to.
    let last_5_trades: [i64; 5] = [100, 101, 99, 102, 103];
    let zeros = [0u64; 10]; // ten zeros
    println!("trades={last_5_trades:?} len={}", last_5_trades.len()); // {:?} = Debug print
    println!("zeros={zeros:?}");

    // Indexing is bounds-checked. Out of range PANICS; it never returns undefined.
    // ❌ UNCOMMENT -> compiles (the index is only known at runtime) but PANICS:
    //   "index out of bounds: the len is 5 but the index is 5"
    let idx: usize = "5".parse().unwrap();
    println!("{}", last_5_trades[idx]);
    // .get(idx) returns Option<&T>, the non-panicking version.
    println!("get(10) = {:?}", last_5_trades.get(10));
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Functions, and expressions vs statements
// ─────────────────────────────────────────────────────────────────────────────

// Parameter types are required, and so is the return type (no inference
// across function signatures, by design: the signature is the contract).
fn spread(bid: i64, ask: i64) -> i64 {
    ask - bid // NO semicolon: this expression is the return value
}

fn mid_price_x2(bid: i64, ask: i64) -> i64 {
    // `return` exists for early exits, but the usual style is to make the
    // last expression the value.
    if bid <= 0 {
        return 0;
    }
    bid + ask // x2 so we stay in integer ticks with no rounding
}

fn section_expressions_vs_statements() {
    println!("\n== expressions vs statements ==");

    println!("spread = {}", spread(10_050, 10_052));
    println!("mid*2  = {}", mid_price_x2(10_050, 10_052));

    // Nearly everything is an expression, including blocks:
    let fee = {
        let notional = 10_000;
        let bps = 5;
        notional * bps / 10_000 // the block's value
    };
    println!("fee = {fee}");

    // THE classic beginner error: a stray semicolon turns the final
    // expression into a statement, so the function returns () instead.
    //
    // ❌ UNCOMMENT -> error[E0308]: mismatched types ... expected `i64`, found `()`
    //    rustc even suggests "remove this semicolon".
    // fn bad_spread(bid: i64, ask: i64) -> i64 {
    //     ask - bid;
    // }
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Control flow: if, loop, while, for, ranges
// ─────────────────────────────────────────────────────────────────────────────
fn section_control_flow() {
    println!("\n== control flow ==");

    // `if` is an expression, so there's no separate ternary operator.
    // Conditions must be bool: no truthiness, so `if 1 {}` won't compile.
    let position: i64 = -3;
    let label = if position > 0 {
        "long"
    } else if position < 0 {
        "short"
    } else {
        "flat"
    };
    println!("position {position} is {label}");
    // Both branches must have the same type:
    // ❌ UNCOMMENT -> error[E0308]: `if` and `else` have incompatible types
    // let bad = if position > 0 { "long" } else { 0 };

    // `loop` is an infinite loop, and `break value` makes it return a value.
    // Here: retry a fake order submit until it succeeds (at most 5 times).
    let mut attempt = 0;
    let filled_on = loop {
        attempt += 1;
        let exchange_accepted = attempt == 3; // pretend 3rd try works
        if exchange_accepted {
            break attempt;
        }
        if attempt >= 5 {
            break -1;
        }
    };
    println!("filled on attempt {filled_on}");

    // `while` works the way you'd expect.
    let mut remaining_qty = 1_000;
    let clip_size = 300;
    let mut clips = 0;
    while remaining_qty > 0 {
        let this_clip = remaining_qty.min(clip_size);
        remaining_qty -= this_clip;
        clips += 1;
    }
    println!("sent {clips} child orders");

    // `for` iterates over anything iterable. Ranges:
    //   0..5   -> 0,1,2,3,4      (end-exclusive)
    //   0..=5  -> 0,1,2,3,4,5    (end-inclusive)
    for level in 0..3 {
        println!("  level {level}");
    }
    let trades = [100, 101, 99];
    for px in trades {
        print!("{px} ");
    }
    println!();
    // Need the index as well? Use .enumerate(), like Array.entries():
    for (i, px) in trades.iter().enumerate() {
        print!("[{i}]={px} ");
    }
    println!();
    // Reverse and step:
    for i in (0..10).rev().step_by(3) {
        print!("{i} ");
    }
    println!();

    // Loop labels let you break out of nested loops.
    'outer: for bid in [100, 101, 102] {
        for ask in [101, 102, 103] {
            if bid >= ask {
                println!("crossed book at bid={bid} ask={ask}");
                break 'outer;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Basic pattern matching (much more in phase 2)
// ─────────────────────────────────────────────────────────────────────────────
fn section_basic_match() {
    println!("\n== match ==");

    // `match` is like a switch with three differences:
    //   - it's an expression (it produces a value)
    //   - it's EXHAUSTIVE: every possible value has to be covered
    //   - it has no fallthrough, so there are no `break`s to forget
    for move_bps in [-250, -20, 0, 15, 400] {
        let label = match move_bps {
            i32::MIN..=-100 => "crash",
            -99..=-1 => "down",
            0 => "flat",
            1..=99 => "up",
            100.. => "moon",
        };
        println!("{move_bps:>5} bps -> {label}");
    }

    // Guards (`if` after the pattern) and `|` for alternatives:
    let hour = 14;
    let session = match hour {
        h if h < 9 => "pre-market",
        9..=15 => "regular",
        16 | 17 | 18 | 19 => "after-hours",
        _ => "closed", // `_` matches anything left
    };
    println!("hour {hour}: {session}");

    // ❌ UNCOMMENT -> error[E0004]: non-exhaustive patterns: `i32::MIN..=-1_i32` ... not covered
    // let bad = match move_bps_example() { 0 => "flat", 1.. => "up" };
}

#[allow(dead_code)]
fn move_bps_example() -> i32 {
    0
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES. Replace each `todo!()` and run `cargo test --bin p01_basics`.
// The tests at the bottom are the spec. Don't edit them.
// ═════════════════════════════════════════════════════════════════════════════

/// Convert a decimal price string with exactly 2 decimals ("650.25") to ticks
/// (65025). You may assume the input is well-formed.
/// Hint: `s.split_once('.')` returns Option<(&str, &str)>; `.unwrap()` it for
/// now. `.parse::<i64>().unwrap()` turns text into a number.
#[allow(dead_code)]
fn price_to_ticks(s: &str) -> i64 {
    let split_s = s.split_once('.');
    match split_s {
        Some((before, after)) => {
            let integer_part = before.parse::<i64>().unwrap();
            let decimal_part = after.parse::<i64>().unwrap();
            integer_part * 100 + decimal_part
        }
        None => 0,
    }
}

/// Volume-weighted average price in integer ticks (truncate the division).
/// vwap = sum(price_i * qty_i) / sum(qty_i). Return 0 if total qty is 0.
#[allow(dead_code)]
fn vwap(prices: [i64; 4], qtys: [i64; 4]) -> i64 {
    let mut volume_weighted_total_price = 0;
    let mut total_qty = 0;
    for (index, price) in prices.iter().enumerate() {
        let qty = qtys[index];
        volume_weighted_total_price += price * qty;
        total_qty += qty;
    }

    if total_qty == 0 {
        return 0;
    }

    volume_weighted_total_price / total_qty
}

/// Return how many fills (taken in order) fit without total qty exceeding
/// `limit`. Stop at the first fill that would exceed it.
/// Try writing it with `loop` + `break value`, then again with `for`.
#[allow(dead_code)]
fn fills_within_limit(fills: [u32; 6], limit: u32) -> usize {
    // let mut total_fill = 0;

    // let mut fill_index: usize = 0;

    // for (index, fill) in fills.iter().enumerate() {
    //     if (total_fill + fill) > limit {
    //         break;
    //     }
    //     total_fill += fill;
    //     fill_index = index + 1;
    // }

    // fill_index

    let mut fill_index: usize = 0;
    let mut total_fill: u32 = 0;
    loop {
        if (fills.len() <= fill_index) {
            return fill_index;
        }

        if total_fill + fills[fill_index] > limit {
            return fill_index;
        }
        total_fill += fills[fill_index];
        fill_index += 1;
    }
}

/// Map a hold time in seconds to a label with a single `match`:
///   0..=59       -> "scalp"
///   60..=3599    -> "intraday" (under an hour)
///   3600..=86399 -> "day"
///   anything else -> "swing"
#[allow(dead_code)]
fn hold_label(secs: u64) -> &'static str {
    if secs <= 59 {
        return "scalp";
    } else if secs <= 3599 {
        return "intraday";
    } else if secs <= 86399 {
        return "day";
    } else {
        return "swing";
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_price_to_ticks() {
        assert_eq!(price_to_ticks("650.25"), 65_025);
        assert_eq!(price_to_ticks("0.01"), 1);
        assert_eq!(price_to_ticks("100.00"), 10_000);
    }

    #[test]
    fn t_vwap() {
        assert_eq!(vwap([100, 102, 101, 0], [10, 10, 20, 0]), 101);
        assert_eq!(vwap([100, 200, 300, 400], [0, 0, 0, 0]), 0);
    }

    #[test]
    fn t_fills_within_limit() {
        assert_eq!(fills_within_limit([10, 20, 30, 40, 50, 60], 60), 3);
        assert_eq!(fills_within_limit([10, 20, 30, 40, 50, 60], 5), 0);
        assert_eq!(fills_within_limit([1, 1, 1, 1, 1, 1], 100), 6);
    }

    #[test]
    fn t_hold_label() {
        assert_eq!(hold_label(5), "scalp");
        assert_eq!(hold_label(60), "intraday");
        assert_eq!(hold_label(3600), "day");
        assert_eq!(hold_label(86_400), "swing");
    }
}
