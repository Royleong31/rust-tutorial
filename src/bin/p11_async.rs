//! PHASE 11 — Async Rust
//!
//!   cargo run  --bin p11_async
//!   cargo test --bin p11_async
//!
//! (The first build downloads Tokio from crates.io and takes a minute.)
//!
//! Threads (phase 10) are great for CPU work. But an exchange gateway mostly
//! WAITS: for the next byte from 10,000 sockets, for a timer, for a reply.
//! An OS thread per connection costs memory (a stack each) and time (the OS
//! switching between them). Async lets ONE thread juggle thousands of
//! waiting jobs, the same way Node does.
//!
//!   Node                                 Rust
//!   JS code                              async fn / async blocks
//!     ↓                                    ↓  (compiled into state machines)
//!   Promise (starts running at once)     Future (does NOTHING until polled)
//!     ↓                                    ↓
//!   event loop (built in, 1 thread)      executor/runtime: Tokio (you pick; many threads)
//!     ↓                                    ↓
//!   libuv                                reactor: mio
//!     ↓                                    ↓
//!   epoll / kqueue / IOCP                epoll / kqueue / IOCP
//!
//! Two big differences from Node:
//!   1. Futures are LAZY. Calling an async fn just builds a value. Nothing
//!      runs until something awaits it.
//!   2. There's no built-in runtime. You choose one (Tokio is the standard)
//!      and it can run tasks on several threads at once, so the phase 10
//!      rules (Send, 'static, no RefCell across threads) apply to tasks too.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::sleep;

// #[tokio::main] turns `async fn main` into a normal `fn main` that starts
// the Tokio runtime (a pool of worker threads, one per core by default) and
// runs this future on it until it finishes.
#[tokio::main]
async fn main() {
    section_futures_are_lazy().await;
    section_what_a_future_is();
    section_tasks().await;
    section_blocking().await;
    section_channels().await;
    section_select_and_timeouts().await;
    section_shared_state().await;
    section_networking().await;
    section_pin();
    puzzles();
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. async/.await, and futures being lazy
// ─────────────────────────────────────────────────────────────────────────────

// An async fn doesn't return a u64. It returns `impl Future<Output = u64>`:
// a value that WILL produce a u64 when it's driven to completion.
async fn place_order(id: u64) -> u64 {
    println!("    placing order {id}");
    sleep(Duration::from_millis(10)).await; // a pause point: other work can run here
    println!("    order {id} acknowledged");
    id * 100
}

async fn section_futures_are_lazy() {
    println!("\n== futures are lazy ==");

    let fut = place_order(1); // nothing printed yet!
    println!("  future created, but no 'placing' line yet");
    println!(
        "  it's a state machine of {} bytes",
        std::mem::size_of_val(&fut)
    );
    let ack = fut.await; // NOW it runs
    println!("  ack = {ack}");

    // In JS, `const p = placeOrder(1)` starts the request immediately, even
    // if you never await p. In Rust, a future you never await (or spawn)
    // never runs. The compiler warns: "futures do nothing unless you
    // `.await` or poll them".
    let _forgotten = place_order(2); // never prints anything

    // `.await` only works inside async code:
    // ❌ UNCOMMENT -> error[E0728]: `await` is only allowed inside `async` functions and blocks
    // fn not_async() { place_order(3).await; }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. What a Future actually is
// ─────────────────────────────────────────────────────────────────────────────
//
//   trait Future {
//       type Output;
//       fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>;
//   }
//   enum Poll<T> { Ready(T), Pending }
//
// An executor calls poll(). The future either finishes (Ready) or says "not
// yet" (Pending). Before returning Pending it hands the Waker from `cx` to
// whatever it's waiting on (a socket, a timer). When that thing is ready, it
// calls waker.wake(), and the executor polls the future again. No busy
// waiting: a sleeping task costs nothing.
//
// The compiler turns each async fn into an enum with one variant per .await
// point, holding the local variables alive at that point. That's the
// "state machine" from section 1, and why its size is known up front.

/// A hand-written future: Pending twice, then Ready.
struct SlowAck {
    polls_left: u32,
}

impl Future for SlowAck {
    type Output = &'static str;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<&'static str> {
        if self.polls_left == 0 {
            Poll::Ready("ack")
        } else {
            self.polls_left -= 1;
            cx.waker().wake_by_ref(); // "poll me again". A real future would wake when data arrives.
            Poll::Pending
        }
    }
}

/// The world's simplest executor: poll in a loop until Ready. Real
/// executors park the thread and wait for wake() instead of spinning.
fn block_on<F: Future>(fut: F) -> (F::Output, u32) {
    let mut fut = std::pin::pin!(fut); // poll needs Pin<&mut F> (section 9)
    let mut cx = Context::from_waker(Waker::noop());
    let mut polls = 0;
    loop {
        polls += 1;
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return (v, polls);
        }
    }
}

fn section_what_a_future_is() {
    println!("\n== what a future is ==");
    let (out, polls) = block_on(SlowAck { polls_left: 2 });
    println!("  hand-written future: {out:?} after {polls} polls");

    // An async block is a future too, and our toy executor can drive it,
    // as long as it doesn't need Tokio's timers or sockets.
    let (out, polls) = block_on(async { SlowAck { polls_left: 1 }.await.len() + 1 });
    println!("  async block around it: {out} after {polls} polls");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Tasks: tokio::spawn, join!, JoinSet
// ─────────────────────────────────────────────────────────────────────────────
//
// .await runs futures ONE AFTER ANOTHER. For concurrency:
//   tokio::join!(a, b)   run several futures concurrently inside the current task
//   tokio::spawn(fut)    start a new TASK: an independent job the runtime
//                        schedules, possibly on another thread. Like
//                        thread::spawn, the future must be Send + 'static.
//   JoinSet              a group of spawned tasks; collect results as they finish
//
// Tasks are cheap: a few hundred bytes each. A million of them is fine. A
// million OS threads isn't.

async fn quote_from(venue: &'static str, delay_ms: u64) -> (&'static str, i64) {
    sleep(Duration::from_millis(delay_ms)).await;
    (venue, 10_000 + delay_ms as i64)
}

async fn section_tasks() {
    println!("\n== tasks ==");

    let t = Instant::now();
    let a = quote_from("A", 30).await;
    let b = quote_from("B", 30).await;
    println!(
        "  sequential: {a:?} {b:?} in ~{} ms",
        t.elapsed().as_millis()
    );

    let t = Instant::now();
    let (a, b) = tokio::join!(quote_from("A", 30), quote_from("B", 30));
    println!(
        "  join!:      {a:?} {b:?} in ~{} ms",
        t.elapsed().as_millis()
    );

    // spawn returns a JoinHandle. Awaiting it gives Result<T, JoinError>
    // (Err if the task panicked or was cancelled), like thread::join.
    let handle = tokio::spawn(quote_from("C", 10));
    println!("  spawned task returned {:?}", handle.await.unwrap());

    let mut set = JoinSet::new();
    for (venue, ms) in [("slow", 40), ("fast", 5), ("mid", 20)] {
        set.spawn(quote_from(venue, ms));
    }
    print!("  JoinSet, in finishing order:");
    while let Some(res) = set.join_next().await {
        print!(" {}", res.unwrap().0);
    }
    println!();

    // A spawned task may outlive this function, so it can't borrow locals:
    // ❌ UNCOMMENT -> error[E0373]: async block may outlive the current function, but it borrows `symbol`, which is owned by the current function
    // let symbol = String::from("BTC-USD");
    // tokio::spawn(async { println!("{symbol}") }).await.unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Never block the runtime
// ─────────────────────────────────────────────────────────────────────────────
//
// A task gives up its thread ONLY at an .await. Code that blocks without
// awaiting (std::thread::sleep, a heavy calculation, std::fs, a blocking DB
// driver) holds the thread, and every other task scheduled on it waits.
// It's the async version of blocking Node's event loop with a long
// synchronous loop.

fn heavy_risk_calc() -> u64 {
    std::thread::sleep(Duration::from_millis(30)); // stands in for CPU-heavy work
    42
}

async fn section_blocking() {
    println!("\n== don't block the runtime ==");

    let t = Instant::now();
    tokio::join!(
        sleep(Duration::from_millis(30)),
        sleep(Duration::from_millis(30)),
        sleep(Duration::from_millis(30))
    );
    println!(
        "  3 × tokio sleep, concurrently:       ~{} ms",
        t.elapsed().as_millis()
    );

    let t = Instant::now();
    tokio::join!(
        async { std::thread::sleep(Duration::from_millis(30)) },
        async { std::thread::sleep(Duration::from_millis(30)) },
        async { std::thread::sleep(Duration::from_millis(30)) },
    );
    println!(
        "  3 × std::thread::sleep, \"concurrently\": ~{} ms (they ran one by one!)",
        t.elapsed().as_millis()
    );

    // For blocking or CPU-heavy work, hand it to Tokio's separate blocking
    // thread pool, and await the result:
    let t = Instant::now();
    let (a, b) = tokio::join!(
        tokio::task::spawn_blocking(heavy_risk_calc),
        tokio::task::spawn_blocking(heavy_risk_calc)
    );
    println!(
        "  2 × spawn_blocking: {} {} in ~{} ms",
        a.unwrap(),
        b.unwrap(),
        t.elapsed().as_millis()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Async channels: one for every messaging pattern
// ─────────────────────────────────────────────────────────────────────────────
//
//   mpsc       many senders, ONE receiver, bounded: send().await waits when
//              full (backpressure). Gateways -> the engine task.
//   oneshot    exactly one value, once. The reply half of a request/response.
//   broadcast  every subscriber gets EVERY message. Market data fan-out.
//              Slow subscribers that fall behind get a Lagged error and skip
//              ahead, rather than slowing everyone down.
//   watch      only the LATEST value is kept. Best bid/ask, config, "shutting down".
//
// The async single writer from phase 10: the engine task owns its state, and
// requests carry a oneshot Sender to reply on.

#[derive(Debug)]
struct NewOrder {
    qty: u64,
    reply: oneshot::Sender<Result<u64, String>>, // where to send the ack
}

async fn section_channels() {
    println!("\n== channels ==");

    // mpsc + oneshot: request/response with a single-writer engine task.
    let (orders_tx, mut orders_rx) = mpsc::channel::<NewOrder>(64);
    let engine = tokio::spawn(async move {
        let mut next_id = 0;
        while let Some(o) = orders_rx.recv().await {
            // None once every Sender is dropped
            let ack = if o.qty == 0 {
                Err("zero qty".to_string())
            } else {
                next_id += 1;
                Ok(next_id)
            };
            let _ = o.reply.send(ack); // the client may have given up; ignore that
        }
    });
    for qty in [5, 0, 3] {
        let (reply_tx, reply_rx) = oneshot::channel();
        orders_tx
            .send(NewOrder {
                qty,
                reply: reply_tx,
            })
            .await
            .unwrap();
        println!("  order qty {qty} -> {:?}", reply_rx.await.unwrap());
    }
    drop(orders_tx); // closes the channel, so the engine's loop ends
    engine.await.unwrap();

    // broadcast: market data to every subscriber.
    let (md_tx, _) = broadcast::channel::<(&str, i64)>(16);
    let mut subscribers = JoinSet::new();
    for name in ["risk", "ui"] {
        let mut rx = md_tx.subscribe(); // subscribe BEFORE sending, or you miss messages
        subscribers.spawn(async move {
            let mut seen = Vec::new();
            while let Ok((sym, px)) = rx.recv().await {
                seen.push(format!("{sym}@{px}"));
            }
            format!("{name} saw {seen:?}")
        });
    }
    for tick in [("BTC", 100), ("BTC", 101)] {
        md_tx.send(tick).unwrap();
    }
    drop(md_tx); // subscribers' recv() returns Err(Closed) after the last message
    while let Some(r) = subscribers.join_next().await {
        println!("  {}", r.unwrap());
    }

    // watch: readers only care about the latest value.
    let (best_tx, mut best_rx) = watch::channel(10_000_i64);
    let reader = tokio::spawn(async move {
        best_rx.changed().await.unwrap(); // wait for the next change
        *best_rx.borrow() // read the CURRENT value (some updates may be skipped)
    });
    sleep(Duration::from_millis(5)).await;
    best_tx.send(10_005).unwrap();
    best_tx.send(10_010).unwrap();
    println!(
        "  watch reader woke up and saw best bid {}",
        reader.await.unwrap()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. select!, timeouts and cancellation
// ─────────────────────────────────────────────────────────────────────────────
//
// select! waits on several futures at once, runs the branch of whichever
// finishes FIRST, and DROPS the others. A feed handler's main loop is
// almost always a select!: the next message, a heartbeat timer, a shutdown
// signal.
//
// Dropping a future CANCELS it: it simply stops at its current .await, and
// its locals are dropped (RAII, phase 3). That's simple, but watch out:
// some futures lose data if they're cancelled halfway. Tokio's docs mark
// each method "cancel safe" or not. mpsc::Receiver::recv is cancel safe.

async fn section_select_and_timeouts() {
    println!("\n== select!, timeouts, cancellation ==");

    let (tx, mut rx) = mpsc::channel::<i64>(16);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        for px in [100, 101, 102] {
            sleep(Duration::from_millis(15)).await;
            tx.send(px).await.unwrap();
        }
        sleep(Duration::from_millis(40)).await;
        let _ = shutdown_tx.send(());
    });

    let mut heartbeat = tokio::time::interval(Duration::from_millis(25));
    let mut beats = 0;
    loop {
        tokio::select! {
            Some(px) = rx.recv() => println!("  price {px}"),
            _ = heartbeat.tick() => beats += 1,
            _ = &mut shutdown_rx => { println!("  shutdown signal"); break; }
        }
    }
    println!("  heartbeats while running: {beats}");

    // timeout: give a future a deadline. Err(Elapsed) if it's too slow, and
    // the slow future is dropped (cancelled).
    let fast = tokio::time::timeout(Duration::from_millis(50), quote_from("fast", 5)).await;
    let slow = tokio::time::timeout(Duration::from_millis(5), quote_from("slow", 50)).await;
    println!("  timeout: fast={fast:?} slow_is_err={}", slow.is_err());

    // Dropping a JoinHandle does NOT stop the task (it keeps running,
    // detached). Use abort().
    let h = tokio::spawn(async {
        sleep(Duration::from_secs(60)).await;
        "never"
    });
    h.abort();
    println!(
        "  aborted task: cancelled = {}",
        h.await.unwrap_err().is_cancelled()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Shared state in async code
// ─────────────────────────────────────────────────────────────────────────────
//
// Arc<Mutex<T>> works in async code too. Which Mutex?
//   std::sync::Mutex    fine, and faster, as long as you NEVER hold the guard
//                       across an .await. Lock, change, unlock, then await.
//   tokio::sync::Mutex  lock().await waits without blocking the thread. Use
//                       it only when you really must hold the lock across an
//                       .await (e.g. one shared connection).
// Even better, as in phase 10: give the state to one task and send it messages.

async fn section_shared_state() {
    println!("\n== shared state ==");
    let positions = Arc::new(Mutex::new(HashMap::<&str, i64>::new()));

    let mut set = JoinSet::new();
    for i in 0..10 {
        let positions = Arc::clone(&positions);
        set.spawn(async move {
            sleep(Duration::from_millis(1)).await; // await BEFORE locking...
            *positions.lock().unwrap().entry("ETH").or_insert(0) += i; // ...lock and unlock on one line
        });
    }
    while set.join_next().await.is_some() {}
    println!(
        "  ETH position = {:?}",
        positions.lock().unwrap().get("ETH")
    );

    // Holding a std MutexGuard across .await. The guard isn't Send, and the
    // task might resume on another thread:
    // ❌ UNCOMMENT -> error: future cannot be sent between threads safely
    // let m = Arc::new(Mutex::new(0));
    // tokio::spawn(async move { let g = m.lock().unwrap(); sleep(Duration::from_millis(1)).await; drop(g); });

    let shared = Arc::new(tokio::sync::Mutex::new(0));
    let s2 = Arc::clone(&shared);
    tokio::spawn(async move {
        let mut g = s2.lock().await; // async lock: waits without blocking the thread
        sleep(Duration::from_millis(1)).await; // allowed with tokio's Mutex
        *g += 1;
    })
    .await
    .unwrap();
    println!("  tokio::sync::Mutex value = {}", *shared.lock().await);
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Async networking: a tiny price server over TCP
// ─────────────────────────────────────────────────────────────────────────────
//
// One task accepts connections, and each connection gets its own task:
// thousands of connections, a handful of threads. The protocol is one line
// per request, which is easy to try by hand with `nc`:
//
//   PRICE BTC-USD   ->  BTC-USD 10050
//   PRICE DOGE-USD  ->  UNKNOWN DOGE-USD
//
// Latest prices live in a `watch` channel. Each connection keeps a Receiver
// and reads the current value whenever asked.

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

async fn handle_connection(
    stream: TcpStream,
    prices: watch::Receiver<HashMap<String, i64>>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split(); // read and write halves, usable separately
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        // `?` works in async fns exactly as in phase 7
        let reply = match line.split_once(' ') {
            Some(("PRICE", sym)) => match prices.borrow().get(sym) {
                Some(px) => format!("{sym} {px}\n"),
                None => format!("UNKNOWN {sym}\n"),
            },
            _ => "ERROR expected: PRICE <symbol>\n".to_string(),
        };
        writer.write_all(reply.as_bytes()).await?;
    }
    Ok(()) // the client closed the connection
}

async fn section_networking() {
    println!("\n== networking ==");
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        // port 0: the OS picks a free port
        Ok(l) => l,
        Err(e) => {
            println!("  couldn't open a local port ({e}); skipping this demo");
            return;
        }
    };
    let addr = listener.local_addr().unwrap();
    println!("  listening on {addr}");

    let (prices_tx, prices_rx) =
        watch::channel(HashMap::from([("BTC-USD".to_string(), 10_050_i64)]));

    // The accept loop runs as its own task.
    let server = tokio::spawn(async move {
        loop {
            let (stream, peer) = listener.accept().await.unwrap();
            let prices = prices_rx.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, prices).await {
                    println!("  connection {peer} failed: {e}");
                }
            });
        }
    });

    // A client, in the same program for the demo.
    let stream = TcpStream::connect(addr).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut replies = BufReader::new(reader).lines();
    for req in ["PRICE BTC-USD", "PRICE DOGE-USD", "HELLO"] {
        writer
            .write_all(format!("{req}\n").as_bytes())
            .await
            .unwrap();
        println!(
            "  > {req:<15} < {}",
            replies.next_line().await.unwrap().unwrap()
        );
    }
    prices_tx.send_modify(|m| {
        m.insert("BTC-USD".into(), 10_060);
    });
    writer.write_all(b"PRICE BTC-USD\n").await.unwrap();
    println!(
        "  > {:<15} < {} (after a price update)",
        "PRICE BTC-USD",
        replies.next_line().await.unwrap().unwrap()
    );

    server.abort(); // stop accepting; open connections end with the program
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Pin, briefly
// ─────────────────────────────────────────────────────────────────────────────
//
// You saw `Pin<&mut Self>` in poll() and `pin!` in block_on. Why?
//
// An async fn's state machine can hold a reference to ANOTHER of its own
// local variables (e.g. a `&buf` alive across an .await). If the state
// machine were moved in memory, that internal reference would point at
// the old location. Pin<P> is a promise: "this value will not move again".
// Once a future is pinned, polling it is safe.
//
// In everyday code you meet Pin in only a few places:
//   - writing poll() by hand (rare)
//   - storing futures of different types together: Pin<Box<dyn Future<Output = T> + Send>>
//   - select! on a future you reuse across loop iterations: `tokio::pin!(fut)`
//     and then `&mut fut` in each iteration.
// The compiler tells you when you need it.

fn section_pin() {
    println!("\n== Pin ==");
    let futs: Vec<Pin<Box<dyn Future<Output = u32> + Send>>> =
        vec![Box::pin(async { 1 }), Box::pin(async { 2 })];
    let total: u32 = futs.into_iter().map(|f| block_on(f).0).sum();
    println!("  two different async blocks, boxed and pinned in one Vec: total {total}");
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. PUZZLES. Each block has a compile error. Uncomment one, read the error,
//     and fix it.
// ─────────────────────────────────────────────────────────────────────────────
fn puzzles() {
    println!("\n== puzzles (uncomment in source) ==");

    // PUZZLE 1: spawn a task that uses an Rc.
    // async fn p1() { let px = std::rc::Rc::new(100); tokio::spawn(async move { sleep(Duration::from_millis(1)).await; println!("{px}"); }); }

    // PUZZLE 2: the type of an async fn's result.
    // async fn best_bid() -> i64 { 100 }
    // async fn p2() { let b: i64 = best_bid(); println!("{b}"); }

    // PUZZLE 3: a task that borrows a local vector.
    // async fn p3() { let fills = vec![1, 2, 3]; tokio::spawn(async { println!("{}", fills.len()); }).await.unwrap(); }

    // PUZZLE 4: count messages in a loop.
    // async fn p4() { let (_tx, rx) = mpsc::channel::<i64>(8); while let Some(px) = rx.recv().await { println!("{px}"); } }
}

// ═════════════════════════════════════════════════════════════════════════════
// EXERCISES: `cargo test --bin p11_async`
// The tests use `#[tokio::test(start_paused = true)]`: Tokio's clock is
// paused and jumps straight to the next timer, so a "5 second" test finishes
// instantly and timings are exact.
// ═════════════════════════════════════════════════════════════════════════════

/// Each venue answers after `delay_ms`. Ask all of them CONCURRENTLY (simulate
/// with `sleep`) and return (name, delay_ms) for each, in the SAME ORDER as
/// the input. Total time must be the slowest venue's delay, not the sum.
/// Hint: spawn one task per venue (the name must become an owned String,
/// since tasks are 'static), keep the JoinHandles in a Vec, then await them
/// in order.
#[allow(dead_code)]
async fn fetch_all(venues: &[(&str, u64)]) -> Vec<(String, u64)> {
    let _ = venues;
    todo!()
}

/// Run `fut`, but give up after `ms` milliseconds with Err("timed out").
#[allow(dead_code)]
async fn with_deadline<T>(fut: impl Future<Output = T>, ms: u64) -> Result<T, &'static str> {
    let _ = (fut, ms);
    todo!()
}

/// A request to the price service: which symbol, and where to send the answer.
#[allow(dead_code)]
struct PriceRequest {
    symbol: String,
    reply: oneshot::Sender<Option<i64>>,
}

/// Spawn a task that OWNS `prices` and answers PriceRequests until every
/// Sender is dropped. Return the Sender for clients (capacity 32 is fine).
#[allow(dead_code)]
fn spawn_price_service(prices: HashMap<String, i64>) -> mpsc::Sender<PriceRequest> {
    let _ = prices;
    todo!()
}

/// Client side: send a request through `svc` and await the reply.
/// Return None if the service is gone or has no price for `symbol`.
#[allow(dead_code)]
async fn get_price(svc: &mpsc::Sender<PriceRequest>, symbol: &str) -> Option<i64> {
    let _ = (svc, symbol);
    todo!()
}

#[derive(Debug, PartialEq)]
#[allow(dead_code)]
enum FeedEnd {
    Closed, // every sender was dropped
    Stale,  // nothing arrived for `stale_after`
}

#[derive(Debug, PartialEq)]
#[allow(dead_code)]
struct FeedReport {
    msgs: u32,
    last: Option<i64>,
    end: FeedEnd,
}

/// Read prices until the feed closes or goes quiet for longer than
/// `stale_after` (measured from the LAST message, or from the start).
/// Count the messages and remember the last price.
/// Hint: loop { match tokio::time::timeout(stale_after, rx.recv()).await { ... } }
#[allow(dead_code)]
async fn monitor_feed(rx: mpsc::Receiver<i64>, stale_after: Duration) -> FeedReport {
    let _ = (rx, stale_after);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant; // Tokio's Instant follows the paused clock

    #[tokio::test(start_paused = true)]
    async fn t_fetch_all() {
        let start = Instant::now();
        let got = fetch_all(&[("slow", 300), ("fast", 100), ("mid", 200)]).await;
        assert_eq!(
            got,
            vec![
                ("slow".to_string(), 300),
                ("fast".to_string(), 100),
                ("mid".to_string(), 200)
            ]
        );
        let took = start.elapsed();
        assert!(
            took >= Duration::from_millis(300) && took < Duration::from_millis(600),
            "took {took:?}: not concurrent?"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn t_with_deadline() {
        assert_eq!(with_deadline(async { 7 }, 10).await, Ok(7));
        assert_eq!(
            with_deadline(sleep(Duration::from_secs(5)), 1_000).await,
            Err("timed out")
        );
        let quick = async {
            sleep(Duration::from_millis(50)).await;
            "done"
        };
        assert_eq!(with_deadline(quick, 100).await, Ok("done"));
    }

    #[tokio::test]
    async fn t_price_service() {
        let svc = spawn_price_service(HashMap::from([
            ("BTC-USD".to_string(), 10_050),
            ("ETH-USD".to_string(), 3_500),
        ]));
        assert_eq!(get_price(&svc, "BTC-USD").await, Some(10_050));
        assert_eq!(get_price(&svc, "DOGE-USD").await, None);
        // Many concurrent clients:
        let mut set = JoinSet::new();
        for _ in 0..20 {
            let svc = svc.clone();
            set.spawn(async move { get_price(&svc, "ETH-USD").await });
        }
        while let Some(r) = set.join_next().await {
            assert_eq!(r.unwrap(), Some(3_500));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn t_monitor_feed_closed() {
        let (tx, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            for px in [100, 101, 102] {
                sleep(Duration::from_millis(10)).await;
                tx.send(px).await.unwrap();
            }
        }); // tx dropped when the task ends
        let r = monitor_feed(rx, Duration::from_millis(50)).await;
        assert_eq!(
            r,
            FeedReport {
                msgs: 3,
                last: Some(102),
                end: FeedEnd::Closed
            }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn t_monitor_feed_stale() {
        let (tx, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            tx.send(100).await.unwrap();
            sleep(Duration::from_millis(30)).await;
            tx.send(99).await.unwrap();
            sleep(Duration::from_secs(10)).await; // goes quiet, but stays open
            drop(tx);
        });
        let start = Instant::now();
        let r = monitor_feed(rx, Duration::from_millis(100)).await;
        assert_eq!(
            r,
            FeedReport {
                msgs: 2,
                last: Some(99),
                end: FeedEnd::Stale
            }
        );
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "should give up after ~130 ms, not wait for the close"
        );
    }
}
