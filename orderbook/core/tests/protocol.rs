//! The text command parser.
//!   cargo test -p orderbook --test protocol

mod common;

use common::*;
use orderbook::{Command, OrderId, ParseError, Qty, Side, TimeInForce, parse_command};

#[test]
fn parses_commands() {
    assert_eq!(
        parse_command("t3 BUY 10 @ 1000"),
        Ok(Command::Submit(lim(3, Side::Buy, 10, 1000)))
    );
    assert_eq!(
        parse_command("  t1 sell 2 @ 995 ioc "),
        Ok(Command::Submit(lim_tif(
            1,
            Side::Sell,
            2,
            995,
            TimeInForce::Ioc
        )))
    );
    assert_eq!(
        parse_command("t2 sell 5 mkt"),
        Ok(Command::Submit(mkt(2, Side::Sell, 5)))
    );
    assert_eq!(parse_command("cancel #4"), Ok(Command::Cancel(OrderId(4))));
    assert_eq!(
        parse_command("amend 4 1"),
        Ok(Command::Amend(OrderId(4), Qty(1)))
    );
    assert_eq!(parse_command("book"), Ok(Command::Show));
}

#[test]
fn reports_what_was_wrong() {
    assert_eq!(parse_command("   "), Err(ParseError::Empty));
    assert!(matches!(
        parse_command("t1 hold 1 @ 5"),
        Err(ParseError::Unknown { what: "side", .. })
    ));
    assert!(matches!(
        parse_command("t1 buy 1 @ 5 gfd"),
        Err(ParseError::Unknown {
            what: "time in force",
            ..
        })
    ));
    assert!(matches!(
        parse_command("t1 buy 1 at 5"),
        Err(ParseError::Unknown {
            what: "order type",
            ..
        })
    ));
    assert!(matches!(
        parse_command("tx buy 1 @ 5"),
        Err(ParseError::BadNumber { what: "trader", .. })
    ));
    assert!(matches!(
        parse_command("t1 buy -1 @ 5"),
        Err(ParseError::BadNumber { what: "qty", .. })
    ));
    assert!(matches!(
        parse_command("flip the table"),
        Err(ParseError::Unknown {
            what: "command",
            ..
        })
    ));
}

#[test]
fn side_and_tif_parse_with_from_str() {
    assert_eq!("SELL".parse::<Side>(), Ok(Side::Sell));
    assert_eq!("post".parse::<TimeInForce>(), Ok(TimeInForce::PostOnly));
    assert!("sideways".parse::<Side>().is_err());
}
