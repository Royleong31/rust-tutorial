# Rust lab (aimed at building an order-book exchange)

One file per phase in `src/bin/`. For each one:

1. **Read** it top to bottom. `main` runs the sections in order.
2. **Run** it: `cargo run --bin p03_ownership`
3. **Break it:** uncomment each `❌ UNCOMMENT` block one at a time, run
   `cargo build`, and read the *whole* error. Put the block back afterwards.
4. **Solve** the `todo!()` exercises at the bottom:
   `cargo test --bin p03_ownership` (the tests are the spec)
5. Ask questions as you go.

| Phase | File | Topic |
|---|---|---|
| 1 | `p01_basics.rs` | variables, integers, ticks vs floats, control flow, `match` |
| 2 | `p02_types.rs` | structs, newtypes, enums, `impl`, `Option`, `Result`, generics, traits |
| 3 | `p03_ownership.rs` | stack/heap, move, `Copy`, `Clone`, moving out of collections, `Drop`/RAII, puzzles |
| 4 | `p04_borrowing.rs` | `&`/`&mut`, shared-XOR-mutable (and races), reborrowing, `*`, slices, borrowing in loops and methods, `split_at_mut`, puzzles |
| 5 | `p05_lifetimes.rs` | dangling refs, `'a` in functions, elision, zero-copy FIX parsing, multiple lifetimes, `'static`, owned vs borrowed design, puzzles |
| 6 | `p06_collections.rs` | `String` vs `&str` vs `str`, `Vec`, closures, iterators, `HashMap` + entry API, `BTreeMap`, `VecDeque`, sets, a first order book |
| 7 | `p07_errors.rs` | panic vs Result, combinators, `?`, custom error enums, `From` + error chains, `Box<dyn Error>`, `TryFrom`, errors in iterators |
| 8 | `p08_traits.rs` | default methods, extension traits, bounds + `where`, operator traits, manual `Ord` + `BinaryHeap`, associated types, static vs dynamic dispatch, `impl Trait`, orphan rule |
| 9 | `p09_smart_pointers.rs` | layout + padding, niches, cache locality, `Box`, fat pointers, `Rc`, `Cell`/`RefCell`, `Weak` + cycles, `Arc`, slab/arena |
| 10 | `p10_concurrency.rs` | threads, scoped threads, `Send`/`Sync`, `Mutex` + poisoning, `RwLock`, channels + backpressure, single-writer engine vs `Arc<Mutex<Book>>`, atomics + memory ordering, deadlocks |
| 11 | `p11_async.rs` | lazy futures, a hand-written `Future` + executor, Tokio tasks, `join!`/`JoinSet`, not blocking the runtime, `mpsc`/`oneshot`/`broadcast`/`watch`, `select!`, timeouts + cancellation, async shared state, a TCP price server, `Pin` |
| 12 | not written yet | modules, workspaces, testing, tracing, serde, benchmarking (when you're ready to build the exchange) |

The capstone order book is split into two crates in [`orderbook/`](orderbook/README.md): a library and a program that runs it on a demo script, an interactive prompt, or a CSV file of orders (`cargo run -p orderbook-cli -- orderbook/data/demo.csv`).

Phase 11 uses Tokio, so its first build downloads crates from crates.io. Its tests pause Tokio's clock, so timing tests run instantly.

Useful extras:
- `cargo clippy --bin <name>`: the linter. It teaches idiomatic Rust.
- `rustc --explain E0382`: the long explanation for any error code.
