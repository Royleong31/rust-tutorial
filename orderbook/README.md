# orderbook

The capstone (`src/bin/capstone_order_book.rs`) split into two crates. The
matching logic is unchanged: the scripted demo prints the same output as the
original.

```
orderbook/
├── core/                  crate `orderbook`: the library. Pure logic, no I/O, no dependencies.
│   ├── src/
│   │   ├── lib.rs         docs, module list, re-exports (the public API)
│   │   ├── types.rs       Price, Qty, OrderId, TraderId
│   │   ├── order.rs       Side, TimeInForce, OrderType, NewOrder, Resting
│   │   ├── event.rs       Event, CancelReason, Reject, EventSink
│   │   ├── protocol.rs    the text command parser
│   │   └── book/
│   │       ├── mod.rs           Book, Level, shared helpers (+ unit tests)
│   │       ├── submit.rs        validate, match, rest
│   │       ├── cancel.rs        cancel, amend
│   │       ├── market_data.rs   best bid/ask, depth, lookups
│   │       └── invariants.rs    check()
│   └── tests/             integration tests: only use the public API
│       ├── common/mod.rs  shared helpers
│       ├── matching.rs    orders.rs    protocol.rs    invariants.rs
├── cli/                   crate `orderbook-cli`, builds the `orderbook` program
│   └── src/
│       ├── main.rs        arguments, picks a mode
│       ├── session.rs     apply/execute commands, Printer, error chains
│       ├── demo.rs        the scripted demo
│       ├── repl.rs        interactive prompt
│       └── csv_input.rs   CSV of orders -> commands (csv + serde)
└── data/demo.csv          the demo script as a CSV file
```

## Run it (from the repo root)

```sh
cargo run -p orderbook-cli                                    # scripted demo
cargo run -p orderbook-cli -- -i                              # type commands yourself
cargo run -p orderbook-cli -- orderbook/data/demo.csv         # run a CSV of orders
cargo run -p orderbook-cli -- my_orders.csv --quiet --tick 1  # errors + summary only
```

## CSV format

```csv
action,order_id,trader,side,qty,price,tif
new,,1,sell,5,1010,gtc     # limit order; tif is gtc, ioc, fok or post (empty = gtc)
new,,2,buy,7,,             # market order: empty price and tif
cancel,3                   # cancel order #3
amend,3,,,2                # reduce order #3 to 2 open
```

Lines starting with `#` are comments, and trailing empty cells can be left off.
Order ids are assigned 1, 2, 3... as orders are accepted; rejected rows don't use one.
A bad row is reported with its line number and skipped.

## Test it

```sh
cargo test -p orderbook                 # unit tests, integration tests, and the doc test
cargo test -p orderbook --test matching # one integration test file
cargo test -p orderbook-cli             # CSV parsing, and "the CSV demo == the scripted demo"
```

## Where networking and threads will go

Only in `cli/` (or a new crate next to it). The library takes commands and
emits events through `EventSink`, so a TCP gateway or an engine thread is just
another place commands come from and events go to. `core/` shouldn't need to change.
