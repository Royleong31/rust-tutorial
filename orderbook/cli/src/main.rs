//! The `orderbook` program: everything that touches the outside world.
//!
//!   cargo run -p orderbook-cli                                  scripted demo
//!   cargo run -p orderbook-cli -- -i                            type commands yourself
//!   cargo run -p orderbook-cli -- orderbook/data/demo.csv       run a CSV file of orders
//!   cargo run -p orderbook-cli -- orders.csv --quiet --tick 1   summary only, tick size 1
//!
//! The matching logic lives in the `orderbook` library crate. This crate
//! decides where commands come from (a script, the terminal, a file) and
//! where events go (the screen). When networking arrives, it'll be one more
//! input source here; the library won't change.

mod csv_input;
mod demo;
mod repl;
mod session;

use orderbook::Book;
use std::path::PathBuf;
use std::process::ExitCode;

/// The capstone's tick size, used unless --tick says otherwise.
const DEFAULT_TICK: i64 = 5;

const USAGE: &str = "\
usage: orderbook [FILE.csv] [-i] [--tick N] [--quiet]

  (no arguments)   run the scripted demo
  FILE.csv         run every row of a CSV file of orders, then print a summary
  -i               interactive: type commands yourself
  --tick N         tick size (default 5)
  --quiet, -q      with a CSV file: print errors and the summary, not every event";

enum Mode {
    Demo,
    Interactive,
    Csv(PathBuf),
    Help,
}

struct Options {
    mode: Mode,
    tick: i64,
    quiet: bool,
}

/// Hand-rolled argument parsing. For anything bigger, the `clap` crate
/// generates this (and --help) from a struct.
fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut opts = Options {
        mode: Mode::Demo,
        tick: DEFAULT_TICK,
        quiet: false,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-i" => opts.mode = Mode::Interactive,
            "-q" | "--quiet" => opts.quiet = true,
            "-h" | "--help" => opts.mode = Mode::Help,
            "--tick" => {
                let n = it.next().ok_or("--tick needs a number")?;
                opts.tick = n
                    .parse()
                    .map_err(|_| format!("--tick: {n:?} is not a number"))?;
                if opts.tick <= 0 {
                    return Err("--tick must be positive".into());
                }
            }
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            path => opts.mode = Mode::Csv(PathBuf::from(path)),
        }
    }
    Ok(opts)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match parse_args(&args) {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("error: {msg}\n\n{USAGE}");
            return ExitCode::from(2); // 2 = "used wrongly", by convention
        }
    };

    let mut book = Book::new(opts.tick);
    match opts.mode {
        Mode::Help => println!("{USAGE}"),
        Mode::Demo => demo::run(&mut book),
        Mode::Interactive => repl::run(&mut book),
        Mode::Csv(path) => match csv_input::run_file(&path, &mut book, opts.quiet) {
            Ok(summary) => summary.print(&book),
            Err(e) => {
                // Only problems with the FILE end up here (missing, unreadable).
                // Bad rows are reported and skipped inside run_file.
                eprintln!("error: {}: {}", path.display(), session::chain(&e));
                return ExitCode::FAILURE;
            }
        },
    }
    ExitCode::SUCCESS
}
