//! Running a CSV file of orders.
//!
//! The file needs a header row. Columns can come in any order, lines that
//! start with `#` are comments, and trailing empty cells can be left off:
//!
//!   action,order_id,trader,side,qty,price,tif
//!   new,,1,sell,5,1010,gtc     limit order (tif: gtc, ioc, fok or post; empty means gtc)
//!   new,,2,buy,7,,             market order: leave price (and tif) empty
//!   cancel,3                   cancel order #3
//!   amend,3,,,2                reduce order #3 to 2 open
//!
//! Order ids are assigned by the book, 1, 2, 3... in the order rows are
//! ACCEPTED (rejected rows don't use one up), so cancel/amend rows refer to
//! those numbers.
//!
//! Text becomes typed values in two steps, both at this boundary:
//!   1. serde turns a CSV record into a CsvRow (strings and numbers, as written)
//!   2. TryFrom<CsvRow> for Command checks the row makes sense
//!
//! A bad row is reported with its line number and skipped; the run goes on.

use crate::session::{Printer, apply, chain};
use orderbook::{
    Book, Command, Event, EventSink, NewOrder, OrderId, ParseError, Price, Qty, Reject, Side,
    TimeInForce, TraderId,
};
use serde::Deserialize;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// One row, as written in the file. serde fills it in by matching header
/// names to field names. Option<T> means the cell may be empty; with
/// `#[serde(default)]` a missing column counts as empty too.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CsvRow {
    action: String,
    order_id: Option<u64>,
    trader: Option<u32>,
    side: Option<String>,
    qty: Option<u64>,
    price: Option<i64>,
    tif: Option<String>,
}

/// A row that parsed as CSV but doesn't describe a valid command.
#[derive(Debug, PartialEq)]
pub enum RowError {
    UnknownAction(String),
    Missing {
        action: &'static str,
        column: &'static str,
    },
    MarketWithTif,
    /// A bad side or tif, from the library's FromStr impls.
    Parse(ParseError),
}

impl fmt::Display for RowError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RowError::UnknownAction(a) => {
                write!(f, "unknown action {a:?} (want new, cancel or amend)")
            }
            RowError::Missing { action, column } => {
                write!(f, "`{action}` needs a value in the {column} column")
            }
            RowError::MarketWithTif => write!(f, "a market order (empty price) can't have a tif"),
            RowError::Parse(_) => write!(f, "bad value"),
        }
    }
}

impl Error for RowError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RowError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ParseError> for RowError {
    fn from(e: ParseError) -> Self {
        RowError::Parse(e)
    }
}

/// Some(v) -> Ok(v), None -> "this action needs this column".
fn need<T>(action: &'static str, column: &'static str, v: Option<T>) -> Result<T, RowError> {
    v.ok_or(RowError::Missing { action, column })
}

impl TryFrom<CsvRow> for Command {
    type Error = RowError;

    fn try_from(row: CsvRow) -> Result<Command, RowError> {
        match row.action.to_ascii_lowercase().as_str() {
            "new" => {
                let trader = TraderId(need("new", "trader", row.trader)?);
                let side: Side = need("new", "side", row.side)?.parse()?; // ParseError -> RowError via From
                let qty = Qty(need("new", "qty", row.qty)?);
                let order = match (row.price, row.tif) {
                    (None, None) => NewOrder::market(trader, side, qty),
                    (None, Some(_)) => return Err(RowError::MarketWithTif),
                    (Some(price), tif) => {
                        let tif = match tif {
                            Some(t) => t.parse()?,
                            None => TimeInForce::Gtc,
                        };
                        NewOrder::limit(trader, side, qty, Price(price), tif)
                    }
                };
                Ok(Command::Submit(order))
            }
            "cancel" => Ok(Command::Cancel(OrderId(need(
                "cancel",
                "order_id",
                row.order_id,
            )?))),
            "amend" => Ok(Command::Amend(
                OrderId(need("amend", "order_id", row.order_id)?),
                Qty(need("amend", "qty", row.qty)?),
            )),
            other => Err(RowError::UnknownAction(other.to_string())),
        }
    }
}

/// Why one row failed. Three layers, three variants.
#[derive(Debug)]
enum RowFailure {
    Read(csv::Error), // not valid CSV, or a cell of the wrong type ("lots" for qty)
    Row(RowError),    // valid CSV, but not a valid command
    Reject(Reject),   // a valid command that the book refused
}

impl fmt::Display for RowFailure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RowFailure::Read(_) => write!(f, "unreadable row"),
            RowFailure::Row(_) => write!(f, "bad row"),
            RowFailure::Reject(_) => write!(f, "rejected"),
        }
    }
}

impl Error for RowFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RowFailure::Read(e) => Some(e),
            RowFailure::Row(e) => Some(e),
            RowFailure::Reject(e) => Some(e),
        }
    }
}

/// An EventSink that counts trades, and prints events unless told not to.
/// It's a second sink built from the first (Printer), which the library
/// neither knows nor cares about.
#[derive(Default)]
struct Tally {
    quiet: bool,
    trades: u64,
    traded: Qty,
    notional: i64,
}

impl EventSink for Tally {
    fn emit(&mut self, e: Event) {
        if let Event::Trade { price, qty, .. } = e {
            self.trades += 1;
            self.traded += qty;
            self.notional += price * qty;
        }
        if !self.quiet {
            Printer.emit(e);
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Summary {
    pub rows: u64,
    pub ok: u64,
    pub failed: u64,
    pub trades: u64,
    pub traded: Qty,
    pub notional: i64,
}

impl Summary {
    pub fn print(&self, book: &Book) {
        println!("\n== summary ==");
        println!(
            "  rows:    {} read, {} ok, {} failed",
            self.rows, self.ok, self.failed
        );
        print!(
            "  trades:  {}, {} lots, notional {}",
            self.trades, self.traded, self.notional
        );
        if self.traded > Qty::ZERO {
            print!(
                ", average price {:.2}",
                self.notional as f64 / self.traded.0 as f64
            );
        }
        println!();
        println!(
            "  book:    best bid {:?}, best ask {:?}, spread {:?}, {} resting orders",
            book.best_bid().map(|p| p.0),
            book.best_ask().map(|p| p.0),
            book.spread(),
            book.len()
        );
        print!("{book}");
    }
}

pub fn run_file(path: &Path, book: &mut Book, quiet: bool) -> Result<Summary, csv::Error> {
    let file = File::open(path)?; // io::Error -> csv::Error, through From
    run(file, book, quiet)
}

/// Run every row from any reader: a File in main, a byte string in tests.
/// Being generic over `Read` is what makes this testable without files.
///
/// Returns Err only for problems with the input as a whole (no header row,
/// an I/O error). Problems with single rows are printed and counted.
pub fn run<R: Read>(input: R, book: &mut Book, quiet: bool) -> Result<Summary, csv::Error> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) // " buy " -> "buy"
        .comment(Some(b'#')) // skip lines starting with #
        .flexible(true) // rows may be shorter than the header
        .from_reader(input);
    let headers = reader.headers()?.clone();

    let mut tally = Tally {
        quiet,
        ..Tally::default()
    };
    let mut summary = Summary::default();

    for result in reader.records() {
        let record = match result {
            Ok(r) => r,
            Err(e) if matches!(e.kind(), csv::ErrorKind::Io(_)) => return Err(e), // the input itself broke
            Err(e) => {
                summary.rows += 1;
                summary.failed += 1;
                println!("    error: {}", chain(&RowFailure::Read(e)));
                continue;
            }
        };
        summary.rows += 1;
        let line = record.position().map_or(0, |p| p.line());
        if !quiet {
            println!(
                "  line {line:>3}> {}",
                record.iter().collect::<Vec<_>>().join(",")
            );
        }

        // Each step's error becomes a RowFailure; and_then stops at the first.
        let outcome = record
            .deserialize::<CsvRow>(Some(&headers))
            .map_err(RowFailure::Read)
            .and_then(|row| Command::try_from(row).map_err(RowFailure::Row))
            .and_then(|cmd| apply(book, cmd, &mut tally).map_err(RowFailure::Reject));

        match outcome {
            Ok(()) => summary.ok += 1,
            Err(e) => {
                summary.failed += 1;
                println!("    error on line {line}: {}", chain(&e));
            }
        }
    }

    summary.trades = tally.trades;
    summary.traded = tally.traded;
    summary.notional = tally.notional;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::DEMO;
    use crate::session::execute;

    fn row(csv_line: &str) -> Result<Command, RowError> {
        let data = format!("action,order_id,trader,side,qty,price,tif\n{csv_line}\n");
        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .flexible(true)
            .from_reader(data.as_bytes());
        let parsed: CsvRow = reader.deserialize().next().unwrap().unwrap();
        Command::try_from(parsed)
    }

    #[test]
    fn rows_become_commands() {
        let t1 = TraderId(1);
        assert_eq!(
            row("new,,1,sell,5,1010,ioc"),
            Ok(Command::Submit(NewOrder::limit(
                t1,
                Side::Sell,
                Qty(5),
                Price(1010),
                TimeInForce::Ioc
            )))
        );
        assert_eq!(
            row("new,,1,BUY,5,1010,"),
            Ok(Command::Submit(NewOrder::limit(
                t1,
                Side::Buy,
                Qty(5),
                Price(1010),
                TimeInForce::Gtc
            )))
        );
        assert_eq!(
            row("NEW, ,1, buy ,5"),
            Ok(Command::Submit(NewOrder::market(t1, Side::Buy, Qty(5))))
        );
        assert_eq!(row("cancel,3"), Ok(Command::Cancel(OrderId(3))));
        assert_eq!(row("amend,3,,,2"), Ok(Command::Amend(OrderId(3), Qty(2))));
    }

    #[test]
    fn bad_rows_say_why() {
        assert_eq!(
            row("buy,,1,buy,5,1010"),
            Err(RowError::UnknownAction("buy".into()))
        );
        assert_eq!(
            row("new,,,buy,5,1010"),
            Err(RowError::Missing {
                action: "new",
                column: "trader"
            })
        );
        assert_eq!(
            row("cancel"),
            Err(RowError::Missing {
                action: "cancel",
                column: "order_id"
            })
        );
        assert_eq!(row("new,,1,buy,5,,ioc"), Err(RowError::MarketWithTif));
        assert!(matches!(
            row("new,,1,hold,5,1010"),
            Err(RowError::Parse(ParseError::Unknown { what: "side", .. }))
        ));
        assert!(matches!(
            row("new,,1,buy,5,1010,gfd"),
            Err(RowError::Parse(ParseError::Unknown {
                what: "time in force",
                ..
            }))
        ));
    }

    #[test]
    fn empty_input_and_missing_columns() {
        let mut book = Book::new(5);
        assert!(run("".as_bytes(), &mut book, true).is_ok_and(|s| s.rows == 0));
        let summary = run("action\nnew,,1,buy,1,1000\n".as_bytes(), &mut book, true).unwrap();
        assert_eq!((summary.ok, summary.failed), (0, 1)); // the row has no trader column to read
    }

    /// The CSV version of the demo must leave the book EXACTLY where the
    /// scripted demo does.
    #[test]
    fn demo_csv_matches_the_scripted_demo() {
        let mut from_csv = Book::new(5);
        let summary = run(
            include_str!("../../data/demo.csv").as_bytes(),
            &mut from_csv,
            true,
        )
        .unwrap();

        let mut from_script = Book::new(5);
        for line in DEMO
            .iter()
            .filter(|l| !l.starts_with("//") && **l != "book")
        {
            let _ = execute(&mut from_script, line, &mut Vec::new());
        }
        from_script
            .submit(
                NewOrder::market(TraderId(9), Side::Buy, Qty(7)),
                &mut Vec::new(),
            )
            .unwrap();

        assert_eq!(from_csv.to_string(), from_script.to_string());
        assert_eq!((summary.rows, summary.ok, summary.failed), (22, 16, 6)); // 4 bad rows + 2 refused (amend up, second cancel)
        assert_eq!(summary.trades, 6); // 2 from the crossing buy, 2 from the IOC, 2 from the market buy
    }
}
