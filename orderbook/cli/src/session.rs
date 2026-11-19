//! Running commands against a book, and showing what happened.
//! Shared by the demo, the interactive prompt and the CSV runner.

use orderbook::{Book, Command, Event, EventSink, HELP, ParseError, Reject, parse_command};
use std::error::Error;
use std::fmt;

/// Print events as they happen. The library only knows the EventSink trait;
/// printing is the program's business, so this sink lives here.
pub struct Printer;

impl EventSink for Printer {
    fn emit(&mut self, e: Event) {
        println!("    {e}");
    }
}

/// Apply a command that changes the book. `book` and `help` don't, so the
/// caller handles them; here they do nothing.
pub fn apply(book: &mut Book, cmd: Command, sink: &mut impl EventSink) -> Result<(), Reject> {
    match cmd {
        Command::Submit(o) => {
            book.submit(o, sink)?;
        }
        Command::Cancel(id) => {
            book.cancel(id, sink)?;
        }
        Command::Amend(id, qty) => book.amend(id, qty, sink)?,
        Command::Show | Command::Help => {}
    }
    Ok(())
}

// Two layers of error, joined by From so `?` converts between them:
//
//   EngineError ── Parse(ParseError) ── BadNumber { source: ParseIntError }
//               └─ Reject(Reject)

#[derive(Debug, PartialEq)]
pub enum EngineError {
    Parse(ParseError),
    Reject(Reject),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EngineError::Parse(_) => write!(f, "could not parse command"),
            EngineError::Reject(_) => write!(f, "rejected"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EngineError::Parse(e) => Some(e),
            EngineError::Reject(e) => Some(e),
        }
    }
}

impl From<ParseError> for EngineError {
    fn from(e: ParseError) -> Self {
        EngineError::Parse(e)
    }
}

impl From<Reject> for EngineError {
    fn from(e: Reject) -> Self {
        EngineError::Reject(e)
    }
}

/// Parse one line of text and run it. The two steps fail with two different
/// error types, and `?` turns both into EngineError through the From impls.
pub fn execute(book: &mut Book, line: &str, sink: &mut impl EventSink) -> Result<(), EngineError> {
    match parse_command(line)? {
        Command::Show => print!("{book}"),
        Command::Help => println!("{HELP}"),
        cmd => apply(book, cmd, sink)?,
    }
    Ok(())
}

/// An error and everything that caused it, on one line: "a: b: c".
pub fn chain(e: &dyn Error) -> String {
    let mut out = e.to_string();
    let mut cause = e.source();
    while let Some(c) = cause {
        out.push_str(&format!(": {c}"));
        cause = c.source();
    }
    out
}

pub fn print_chain(e: &dyn Error) {
    println!("    error: {}", chain(e));
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderbook::OrderId;

    #[test]
    fn execute_errors_chain() {
        let mut b = Book::new(5);
        let mut ev = Vec::new();
        execute(&mut b, "t1 sell 3 @ 1005", &mut ev).unwrap();

        let e = execute(&mut b, "t1 buy lots @ 1000", &mut ev).unwrap_err();
        assert!(matches!(
            e,
            EngineError::Parse(ParseError::BadNumber { what: "qty", .. })
        ));
        // EngineError -> ParseError -> ParseIntError
        assert!(e.source().and_then(|p| p.source()).is_some());
        assert_eq!(
            chain(&e),
            "could not parse command: qty is not a valid number: invalid digit found in string"
        );

        let e = execute(&mut b, "cancel 99", &mut ev).unwrap_err();
        assert_eq!(e, EngineError::Reject(Reject::UnknownOrder(OrderId(99))));

        ev.clear();
        execute(&mut b, "t9 buy 1 @ 1005", &mut ev).unwrap();
        assert!(ev.iter().any(|e| matches!(
            e,
            Event::Trade {
                maker: OrderId(1),
                ..
            }
        )));
    }
}
