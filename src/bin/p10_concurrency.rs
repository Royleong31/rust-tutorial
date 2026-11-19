//! PHASE 10 — Concurrency with threads
//!
//!   cargo run  --bin p10_concurrency
//!   cargo test --bin p10_concurrency
//!
//! Node gives you one thread and an event loop, so two pieces of your JS
//! never run at the same instant. Rust gives you real OS threads that run in
//! PARALLEL on different CPU cores. That's faster, and it's exactly where
//! C++ and Java programs get data races.
//!
//! Rust prevents data races at COMPILE time, using rules you already know:
//!   - ownership (phase 3): moving a value into a thread means nobody else has it
//!   - borrowing (phase 4): many readers OR one writer, now across threads
//!   - two marker traits, Send and Sync (section 3), that say which types are
//!     safe to move or share between threads
//!
//! Then comes architecture. The classic `Arc<Mutex<OrderBook>>` works, but
//! for a matching engine a SINGLE WRITER thread fed by channels (section 6)
//! is usually better. It's the same idea as your Node event loop, but you
//! choose where it runs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

fn main() {
    section_threads();
    section_scoped_threads();
    section_send_and_sync();
    section_mutex();
    section_rwlock();
    section_channels();
    section_single_writer();
    section_atomics();
    section_common_problems();
    puzzles();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Buy,
    Sell,
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Spawning threads
// ─────────────────────────────────────────────────────────────────────────────

fn section_threads() {
    println!("\n== threads ==");
    let cores = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("  this machine can run {cores} threads in parallel");

    // thread::spawn takes a closure and runs it on a NEW OS thread. It
    // returns a JoinHandle right away; the thread runs alongside this one.
    let symbol = String::from("BTC-USD");
    let handle = thread::spawn(move || {
        // `move` is required: the thread may outlive this function, so it
        // can't borrow `symbol` (phase 5's 'static rule). It takes ownership.
        thread::sleep(Duration::from_millis(20)); // pretend to do work
        format!("loaded {symbol}")
    });
    println!("  main thread keeps going while the worker sleeps...");

    // join() waits for the thread to finish and hands back its return value,
    // wrapped in a Result: Err means the thread panicked.
    let msg = handle.join().expect("worker panicked");
    println!("  worker returned: {msg}");

    // A panic stays inside its thread; join() reports it to whoever waits.
    println!("  (the 'panicked at' message below is expected)");
    let bad = thread::Builder::new()
        .name("risk-worker".into()) // named threads make crash logs readable
        .spawn(|| panic!("position limit table missing"))
        .unwrap();
    println!(
        "  joined the panicking thread: is_err = {}",
        bad.join().is_err()
    );

    // Without `move`, the closure would try to BORROW `symbol`:
    // ❌ UNCOMMENT -> error[E0373]: closure may outlive the current function, but it borrows `venue`, which is owned by the current function
    // let venue = String::from("paper");
    // thread::spawn(|| println!("{venue}")).join().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Scoped threads: borrowing local data safely
// ─────────────────────────────────────────────────────────────────────────────
//
// thread::scope guarantees every thread started inside it has finished
// before the scope returns. So those threads CAN borrow local variables:
// the compiler knows the locals outlive the threads. It's the easiest way to
// split work across cores.

fn section_scoped_threads() {
    println!("\n== scoped threads ==");
    let fills: Vec<(i64, u64)> = (0..1_000)
        .map(|i| (100 + i % 7, 1 + i as u64 % 3))
        .collect();

    let halves: Vec<&[(i64, u64)]> = fills.chunks(fills.len() / 2).collect();
    let (a, b) = thread::scope(|s| {
        // Both threads BORROW `fills` (through the slices). No Arc, no clone.
        let h1 = s.spawn(|| halves[0].iter().map(|(p, q)| p * *q as i64).sum::<i64>());
        let h2 = s.spawn(|| halves[1].iter().map(|(p, q)| p * *q as i64).sum::<i64>());
        (h1.join().unwrap(), h2.join().unwrap())
    }); // both threads are guaranteed done here
    let serial: i64 = fills.iter().map(|(p, q)| p * *q as i64).sum();
    println!("  parallel {} + {} = {}, serial {serial}", a, b, a + b);

    // Borrowing rules still apply. Two threads can't both get &mut:
    // ❌ UNCOMMENT -> error[E0499]: cannot borrow `total` as mutable more than once at a time
    // let mut total = 0;
    // thread::scope(|s| { s.spawn(|| total += 1); s.spawn(|| total += 1); });
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Send and Sync: how the compiler knows what's thread-safe
// ─────────────────────────────────────────────────────────────────────────────
//
//   Send  a value of this type can be MOVED to another thread.
//   Sync  a value of this type can be SHARED (&T) between threads at the
//         same time. (Formally: T is Sync if &T is Send.)
//
// You never implement these yourself. The compiler works them out: a struct
// is Send/Sync if all its fields are. Almost everything is both. The
// exceptions are exactly the types from phase 9 that cut corners for speed:
//
//   Rc<T>       neither. Its counter isn't atomic (phase 9, section 7).
//   Cell/RefCell  Send but NOT Sync. Their checks aren't thread-safe.
//   Arc<T>      Send + Sync, if T is Send + Sync
//   Mutex<T>    Send + Sync, if T is Send. The lock makes sharing safe.
//
// thread::spawn requires its closure (and so everything it captures) to be
// Send + 'static. That one bound is what stops data races from compiling.

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

fn section_send_and_sync() {
    println!("\n== Send and Sync ==");
    assert_send::<Vec<String>>();
    assert_sync::<Arc<Mutex<Vec<String>>>>();
    assert_send::<std::cell::RefCell<i64>>(); // can be moved to a thread...
    println!("  checked at compile time: Vec, Arc<Mutex<..>>, RefCell are fine where used");

    // ...but not shared between threads:
    // ❌ UNCOMMENT -> error[E0277]: `RefCell<i64>` cannot be shared between threads safely
    // assert_sync::<std::cell::RefCell<i64>>();

    // The real-world version: trying to share a RefCell through an Arc.
    // ❌ UNCOMMENT -> error[E0277]: `RefCell<i64>` cannot be shared between threads safely
    // let shared = Arc::new(std::cell::RefCell::new(0_i64));
    // let s2 = Arc::clone(&shared);
    // thread::spawn(move || *s2.borrow_mut() += 1).join().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Mutex<T>: one thread at a time
// ─────────────────────────────────────────────────────────────────────────────
//
// A Mutex WRAPS the data it protects. The only way to reach the data is
// lock(), which waits until no other thread holds the lock, then returns a
// GUARD. The guard derefs to &mut T, and unlocks when it's dropped (RAII,
// phase 3). So you can't forget to unlock, and you can't touch the data
// without locking. In most languages the lock and the data are separate,
// and nothing stops you using the data unlocked.
//
// To share the Mutex between threads it needs several owners: Arc (phase 9).

fn section_mutex() {
    println!("\n== Mutex ==");
    let positions: Arc<Mutex<HashMap<&str, i64>>> = Arc::new(Mutex::new(HashMap::new()));

    let mut handles = vec![];
    for t in 0..4 {
        let positions = Arc::clone(&positions);
        handles.push(thread::spawn(move || {
            for _ in 0..1_000 {
                let mut map = positions.lock().unwrap(); // blocks while another thread holds it
                *map.entry("BTC-USD").or_insert(0) += if t % 2 == 0 { 2 } else { -1 };
            } // guard dropped at the end of each iteration: unlocked
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    println!(
        "  BTC-USD position: {:?} (expected 2000)",
        positions.lock().unwrap().get("BTC-USD")
    );

    // Keep the critical section SHORT. Copy out what you need, drop the
    // guard, then do slow work (I/O, logging, computing) without the lock.
    let snapshot: i64 = {
        let map = positions.lock().unwrap();
        *map.get("BTC-USD").unwrap_or(&0)
    }; // unlocked here
    println!("  snapshot taken, lock already released: {snapshot}");

    // POISONING: if a thread panics while holding the lock, the data might
    // be half-updated. The Mutex remembers, and later lock() calls return
    // Err. `.unwrap()` on lock() is the common choice: "if another thread
    // crashed mid-update, crash too". into_inner() lets you recover the data.
    let m = Arc::new(Mutex::new(0));
    let m2 = Arc::clone(&m);
    println!("  (the 'panicked at' message below is expected)");
    let _ = thread::spawn(move || {
        let _guard = m2.lock().unwrap();
        panic!("crashed while holding the lock");
    })
    .join();
    match m.lock() {
        Ok(v) => println!("  lock ok: {v}"),
        Err(poisoned) => println!(
            "  lock is poisoned; recovered value = {}",
            *poisoned.into_inner()
        ),
    }

    // Without the Mutex, sharing mutable data doesn't compile at all:
    // ❌ UNCOMMENT -> error[E0594]: cannot assign to data in an `Arc`
    // let counter = Arc::new(0);
    // let c2 = Arc::clone(&counter);
    // thread::spawn(move || *c2 += 1).join().unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. RwLock<T>: many readers OR one writer
// ─────────────────────────────────────────────────────────────────────────────
//
// The phase 4 rule again, enforced at runtime by blocking: any number of
// read() guards at once, or a single write() guard. It suits data that's
// read constantly and changed rarely, like instrument settings or fee
// tables. For data written as often as it's read, a plain Mutex is simpler,
// and often faster.

#[derive(Debug, Clone)]
struct InstrumentConfig {
    tick: i64,
    max_qty: u64,
}

fn section_rwlock() {
    println!("\n== RwLock ==");
    let config = Arc::new(RwLock::new(InstrumentConfig {
        tick: 5,
        max_qty: 100,
    }));

    thread::scope(|s| {
        for i in 0..3 {
            let config = &config; // scoped threads can borrow the Arc itself
            s.spawn(move || {
                let c = config.read().unwrap(); // readers don't block each other
                println!("  reader {i}: tick {} max_qty {}", c.tick, c.max_qty);
            });
        }
    });

    config.write().unwrap().max_qty = 50; // the writer waits for all readers to finish
    println!("  after update: {:?}", config.read().unwrap());
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Channels: share by communicating
// ─────────────────────────────────────────────────────────────────────────────
//
// "Don't communicate by sharing memory; share memory by communicating."
// A channel MOVES values from one thread to another. Once sent, the sender
// doesn't have the value any more (ownership again), so there's nothing to
// lock.
//
// std::sync::mpsc = Multiple Producer, Single Consumer:
//   channel()       unbounded: send never blocks, so the queue can grow without limit
//   sync_channel(n) bounded: send BLOCKS when n messages are waiting. That's
//                   BACKPRESSURE: a slow consumer slows the producers down
//                   instead of letting memory grow until the process dies.

fn section_channels() {
    println!("\n== channels ==");
    let (tx, rx) = mpsc::channel::<String>();

    for venue in ["NYSE", "NASDAQ", "CBOE"] {
        let tx = tx.clone(); // each producer gets its own Sender
        thread::spawn(move || {
            tx.send(format!("hello from {venue}")).unwrap();
        });
    }
    drop(tx); // drop the original Sender, or the loop below would wait forever

    // Iterating the Receiver yields messages until EVERY Sender is dropped.
    let mut got: Vec<String> = rx.iter().collect();
    got.sort(); // arrival order is up to the OS scheduler
    println!("  {got:?}");

    // A sent value is MOVED:
    // ❌ UNCOMMENT -> error[E0382]: borrow of moved value: `order`
    // let (tx2, _rx2) = mpsc::channel::<String>();
    // let order = String::from("BUY 5 BTC-USD");
    // tx2.send(order).unwrap();
    // println!("{order}");

    // Bounded channel with capacity 2: the producer runs ahead by at most 2.
    let (tx, rx) = mpsc::sync_channel::<u32>(2);
    let producer = thread::spawn(move || {
        for i in 0..5 {
            tx.send(i).unwrap(); // blocks once 2 are waiting
        }
    });
    thread::sleep(Duration::from_millis(10));
    let received: Vec<u32> = rx.iter().collect();
    producer.join().unwrap();
    println!("  bounded channel delivered {received:?}");
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Architecture: Arc<Mutex<Book>> vs a single writer
// ─────────────────────────────────────────────────────────────────────────────
//
// OPTION A: every gateway thread locks the shared book and matches inline.
//
//   gateway 1 ─┐
//   gateway 2 ─┼──► lock(Arc<Mutex<OrderBook>>) ──► match
//   gateway 3 ─┘
//
//   Simple, but: threads spend their time waiting on the lock; the lock is
//   held during matching, so a big sweep stalls everyone; the order in
//   which orders get processed depends on the OS scheduler, so you can't
//   replay a day and get the same result; and every extra lock (risk,
//   positions) adds deadlock risk.
//
// OPTION B: ONE engine thread OWNS the book. Gateways send commands over a
// channel; the engine processes them one at a time and publishes events.
//
//   gateway 1 ─┐                 ┌──────────────────────────┐
//   gateway 2 ─┼── commands ───► │ engine thread            │ ──► events (fills, acks)
//   gateway 3 ─┘    (channel)    │ owns OrderBook, no locks │     (channel)
//                                └──────────────────────────┘
//
//   No locks in the matching code at all: it's ordinary single-threaded
//   Rust with &mut self. Every command gets a sequence number, so the
//   command log is a total order: replay it and you get the same book
//   (event sourcing). Real exchanges (LMAX, most crypto venues) work this
//   way. It's your Node event loop, but you decide what runs on it, and the
//   gateways, risk checks and market data run in parallel around it.

#[derive(Debug)]
enum Command {
    Place { client: u32, side: Side, qty: u64 },
    Stop,
}

#[derive(Debug)]
struct Event {
    seq: u64,
    client: u32,
    new_position: i64,
}

fn section_single_writer() {
    println!("\n== single writer ==");
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
    let (evt_tx, evt_rx) = mpsc::channel::<Event>();

    // The engine: owns its state outright. No Arc, no Mutex.
    let engine = thread::spawn(move || {
        let mut positions: HashMap<u32, i64> = HashMap::new();
        let mut seq = 0;
        for cmd in cmd_rx {
            match cmd {
                Command::Place { client, side, qty } => {
                    seq += 1;
                    let pos = positions.entry(client).or_insert(0);
                    *pos += if side == Side::Buy {
                        qty as i64
                    } else {
                        -(qty as i64)
                    };
                    evt_tx
                        .send(Event {
                            seq,
                            client,
                            new_position: *pos,
                        })
                        .unwrap();
                }
                Command::Stop => break,
            }
        }
        positions // the final state goes back to whoever joins
    });

    // Three gateways sending at the same time.
    let gateways: Vec<_> = (1..=3)
        .map(|client| {
            let tx = cmd_tx.clone();
            thread::spawn(move || {
                for i in 0..3 {
                    let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
                    tx.send(Command::Place {
                        client,
                        side,
                        qty: 10 * client as u64,
                    })
                    .unwrap();
                }
            })
        })
        .collect();
    for g in gateways {
        g.join().unwrap();
    }
    cmd_tx.send(Command::Stop).unwrap();
    let final_positions = engine.join().unwrap();

    let events: Vec<Event> = evt_rx.iter().collect();
    // One total order across all gateways: seq 1, 2, 3... with no gaps.
    let timeline: Vec<String> = events
        .iter()
        .map(|e| format!("#{}:c{}={}", e.seq, e.client, e.new_position))
        .collect();
    println!(
        "  {} events in one sequence: {}",
        events.len(),
        timeline.join(" ")
    );
    let mut fp: Vec<_> = final_positions.into_iter().collect();
    fp.sort();
    println!("  final positions: {fp:?}");
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Atomics and memory ordering basics
// ─────────────────────────────────────────────────────────────────────────────
//
// Atomic types (AtomicU64, AtomicBool, ...) are integers that several
// threads can update without a lock, using special CPU instructions.
// They're perfect for counters, id generators, and flags. Note that the
// methods take &self: they're another form of interior mutability.

static NEXT_ORDER_ID: AtomicU64 = AtomicU64::new(1);

fn new_order_id() -> u64 {
    // fetch_add: add 1 and return the OLD value, as ONE indivisible step.
    // `x += 1` on a shared u64 is three steps (read, add, write), and two
    // threads interleaving them lose updates.
    NEXT_ORDER_ID.fetch_add(1, Ordering::Relaxed)
}

fn section_atomics() {
    println!("\n== atomics ==");
    let ids: Vec<u64> = thread::scope(|s| {
        let hs: Vec<_> = (0..4)
            .map(|_| s.spawn(|| (0..250).map(|_| new_order_id()).collect::<Vec<_>>()))
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    println!(
        "  4 threads took {} ids, {} unique",
        ids.len(),
        sorted.len()
    );

    // MEMORY ORDERING. CPUs and compilers reorder memory operations for
    // speed. Within one thread you can't tell, but ANOTHER thread can see
    // writes in a different order than you made them. The Ordering argument
    // says how much reordering is allowed:
    //
    //   Relaxed  only the atomic itself is consistent. Fine for counters and
    //            statistics, where nothing else depends on the value.
    //   Release (on a store) + Acquire (on a load)
    //            "publish". Everything written BEFORE the Release store is
    //            visible to a thread AFTER its Acquire load sees that store.
    //   SeqCst   the strictest: one global order everyone agrees on. The safe
    //            default when you're unsure. It's slightly slower.
    //
    // The publish pattern: write the data, then raise a flag with Release.
    // The reader waits for the flag with Acquire, then reads the data.
    let snapshot = AtomicU64::new(0); // stands in for a bigger piece of data
    let ready = AtomicBool::new(false);
    thread::scope(|s| {
        s.spawn(|| {
            snapshot.store(10_050, Ordering::Relaxed); // 1. write the data
            ready.store(true, Ordering::Release); // 2. publish it
        });
        s.spawn(|| {
            while !ready.load(Ordering::Acquire) {
                std::hint::spin_loop(); // tell the CPU we're busy-waiting
            }
            // Guaranteed to see 10_050, never 0, because of Release/Acquire.
            println!("  reader saw best bid {}", snapshot.load(Ordering::Relaxed));
        });
    });
    // In practice: use Mutex and channels first. Reach for atomics for
    // counters and flags, and only write lock-free data structures when a
    // profiler says you must. They're very hard to get right.
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Common problems (read, don't run)
// ─────────────────────────────────────────────────────────────────────────────
//
// DEADLOCK. Rust prevents data races, NOT deadlocks. Two threads, two locks,
// taken in opposite orders:
//
//     thread 1: let a = acct_a.lock(); let b = acct_b.lock();   // holds A, waits for B
//     thread 2: let b = acct_b.lock(); let a = acct_a.lock();   // holds B, waits for A
//
// Both wait forever. The fix is to always take locks in ONE global order
// (for example, lowest account id first). Exercise 5 has you do exactly that.
// Locking the same std Mutex twice in one thread also deadlocks (or panics).
//
// GUARD DROPPED TOO EARLY. `let _ = m.lock().unwrap();` drops the guard on
// the same line (phase 3), so nothing is actually protected. Use
// `let _guard = ...`.
//
// GUARD HELD TOO LONG. A guard lives until the end of its scope. Holding it
// while you log, do I/O or sleep blocks every other thread. Copy out, then drop.
//
// FALSE SHARING. Two atomics updated by different threads but sitting in
// the same 64-byte cache line (phase 9) make the cores fight over that line.
// Hot per-thread counters are often padded to 64 bytes apart.

fn section_common_problems() {
    println!("\n== common problems ==");
    println!("  read section 9 in the source");
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. PUZZLES. Each block has a compile error. Uncomment one, read the error,
//     and fix it.
// ─────────────────────────────────────────────────────────────────────────────
fn puzzles() {
    println!("\n== puzzles (uncomment in source) ==");

    // PUZZLE 1: two threads each add a fill to a shared Vec.
    // let fills = Arc::new(Vec::new());
    // let f2 = Arc::clone(&fills);
    // thread::spawn(move || f2.push(1)).join().unwrap();

    // PUZZLE 2: send a quote to a worker thread.
    // let quote = std::rc::Rc::new(100);
    // thread::spawn(move || println!("{quote}")).join().unwrap();

    // PUZZLE 3: count orders in a background thread, then print the count.
    // let mut count = 0;
    // let h = thread::spawn(|| { count += 1; });
    // h.join().unwrap();
    // println!("{count}");

    // PUZZLE 4: use a Mutex shared by two threads, without Arc.
    // let book = Mutex::new(vec![100]);
    // let h1 = thread::spawn(move || book.lock().unwrap().push(101));
    // let h2 = thread::spawn(move || book.lock().unwrap().push(102));
    // h1.join().unwrap(); h2.join().unwrap();
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES: `cargo test --bin p10_concurrency`
// ═════════════════════════════════════════════════════════════════════════════

/// Sum price * qty over all fills, using `n_threads` scoped threads (each
/// takes one chunk). Must equal the single-threaded sum.
/// Hint: chunk size = len.div_ceil(n_threads) (and at least 1);
/// thread::scope; each thread returns its partial sum; add up the joins.
#[allow(dead_code)]
fn parallel_notional(fills: &[(i64, u64)], n_threads: usize) -> i64 {
    let _ = (fills, n_threads);
    todo!()
}

/// A thread-safe id generator. The first id is 1. Many threads may call
/// `next_id` at once through a shared reference, and every id must be unique.
#[allow(dead_code)]
struct IdGen {
    next: AtomicU64,
}

#[allow(dead_code)]
impl IdGen {
    fn new() -> Self {
        IdGen {
            next: AtomicU64::new(1),
        }
    }

    fn next_id(&self) -> u64 {
        todo!()
    }
}

/// Positions per symbol, safe to update from many threads through &self.
#[allow(dead_code)]
struct SharedPositions {
    inner: Mutex<HashMap<String, i64>>,
}

#[allow(dead_code)]
impl SharedPositions {
    fn new() -> Self {
        SharedPositions {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Add `delta` to the symbol's position (starting from 0).
    fn apply(&self, symbol: &str, delta: i64) {
        let _ = (symbol, delta);
        todo!()
    }

    /// The current position, 0 if the symbol was never traded.
    fn get(&self, symbol: &str) -> i64 {
        let _ = symbol;
        todo!()
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum LedgerCmd {
    Deposit { client: u32, amount: i64 },
    Withdraw { client: u32, amount: i64 },
}

#[derive(Debug, Default, PartialEq)]
#[allow(dead_code)]
struct LedgerResult {
    balances: HashMap<u32, i64>,
    rejected: u32, // withdrawals larger than the client's balance
}

/// Start a single-writer ledger thread. It owns the balances, applies
/// commands in arrival order, and REJECTS (counts, without applying) any
/// withdrawal larger than the client's current balance. It stops when every
/// Sender has been dropped, and returns its final state from the thread.
/// Hint: `for cmd in rx { ... }` ends by itself when the senders are gone.
#[allow(dead_code)]
fn spawn_ledger() -> (mpsc::Sender<LedgerCmd>, thread::JoinHandle<LedgerResult>) {
    todo!()
}

#[derive(Debug)]
#[allow(dead_code)]
struct Acct {
    id: u32,
    cash: i64,
}

/// Move `amount` from `from` to `to`, where both may be locked by other
/// threads doing transfers in the OPPOSITE direction. Avoid deadlock by
/// always locking the account with the LOWER id first. Return
/// Err("same account") if both are the same Mutex (std::ptr::eq), and
/// Err("insufficient funds") if `from` can't cover it.
///
/// Trap: to read the ids you have to lock. Read each id in its OWN statement,
/// so the guard is dropped before you lock the other account. Writing
/// `from.lock().unwrap().id < to.lock().unwrap().id` keeps BOTH guards alive
/// until the end of the statement, which locks them in the wrong order.
#[allow(dead_code)]
fn transfer(from: &Mutex<Acct>, to: &Mutex<Acct>, amount: i64) -> Result<(), &'static str> {
    let _ = (from, to, amount);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn t_parallel_notional() {
        let fills: Vec<(i64, u64)> = (0..10_001)
            .map(|i| (100 + i % 13, 1 + (i % 5) as u64))
            .collect();
        let serial: i64 = fills.iter().map(|(p, q)| p * *q as i64).sum();
        for n in [1, 2, 3, 8] {
            assert_eq!(parallel_notional(&fills, n), serial, "n_threads = {n}");
        }
        assert_eq!(parallel_notional(&[], 4), 0);
    }

    #[test]
    fn t_id_gen() {
        let g = IdGen::new();
        let all: Vec<u64> = thread::scope(|s| {
            let hs: Vec<_> = (0..8)
                .map(|_| s.spawn(|| (0..1_000).map(|_| g.next_id()).collect::<Vec<_>>()))
                .collect();
            hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        let unique: HashSet<u64> = all.iter().copied().collect();
        assert_eq!(unique.len(), 8_000);
        assert_eq!(unique.iter().min(), Some(&1));
        assert_eq!(unique.iter().max(), Some(&8_000));
    }

    #[test]
    fn t_shared_positions() {
        let p = Arc::new(SharedPositions::new());
        let hs: Vec<_> = (0..4)
            .map(|t| {
                let p = Arc::clone(&p);
                thread::spawn(move || {
                    for _ in 0..500 {
                        p.apply("ETH-USD", if t < 2 { 3 } else { -1 });
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(p.get("ETH-USD"), 2_000);
        assert_eq!(p.get("SOL-USD"), 0);
    }

    #[test]
    fn t_ledger() {
        let (tx, handle) = spawn_ledger();
        tx.send(LedgerCmd::Deposit {
            client: 1,
            amount: 100,
        })
        .unwrap();
        tx.send(LedgerCmd::Withdraw {
            client: 1,
            amount: 30,
        })
        .unwrap();
        tx.send(LedgerCmd::Withdraw {
            client: 1,
            amount: 80,
        })
        .unwrap(); // rejected: only 70 left
        tx.send(LedgerCmd::Withdraw {
            client: 2,
            amount: 1,
        })
        .unwrap(); // rejected: client 2 has 0

        // Several producers at once:
        thread::scope(|s| {
            for _ in 0..4 {
                let tx = tx.clone();
                s.spawn(move || {
                    for _ in 0..25 {
                        tx.send(LedgerCmd::Deposit {
                            client: 3,
                            amount: 1,
                        })
                        .unwrap();
                    }
                });
            }
        });
        drop(tx);
        let result = handle.join().unwrap();
        assert_eq!(result.balances.get(&1), Some(&70));
        assert_eq!(result.balances.get(&3), Some(&100));
        assert_eq!(result.rejected, 2);
    }

    #[test]
    fn t_transfer_no_deadlock() {
        let a = Arc::new(Mutex::new(Acct {
            id: 1,
            cash: 1_000_000,
        }));
        let b = Arc::new(Mutex::new(Acct {
            id: 2,
            cash: 1_000_000,
        }));
        let (done_tx, done_rx) = mpsc::channel();
        for dir in 0..2 {
            let (a, b, done_tx) = (Arc::clone(&a), Arc::clone(&b), done_tx.clone());
            thread::spawn(move || {
                for _ in 0..20_000 {
                    let r = if dir == 0 {
                        transfer(&a, &b, 1)
                    } else {
                        transfer(&b, &a, 1)
                    };
                    r.unwrap();
                }
                done_tx.send(()).unwrap();
            });
        }
        drop(done_tx); // only the threads hold senders now, so a panicked thread is noticed at once
        for _ in 0..2 {
            // A deadlocked solution fails here after 10 s instead of hanging forever.
            done_rx
                .recv_timeout(Duration::from_secs(10))
                .expect("a transfer thread panicked or deadlocked");
        }
        assert_eq!(a.lock().unwrap().cash + b.lock().unwrap().cash, 2_000_000);
        assert_eq!(transfer(&a, &a, 1), Err("same account"));
        assert_eq!(transfer(&a, &b, 5_000_000), Err("insufficient funds"));
    }
}
