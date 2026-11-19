//! The interactive prompt.

use crate::session::{Printer, execute, print_chain};
use orderbook::{Book, HELP};
use std::io::{self, Write};

pub fn run(book: &mut Book) {
    println!("== order book (tick size {}) ==\n{HELP}", book.tick());
    // One String reused for every line, instead of allocating a new one each time.
    let mut line = String::new();
    loop {
        print!("> ");
        io::stdout().flush().ok(); // print! doesn't flush on its own
        line.clear();
        match io::stdin().read_line(&mut line) {
            Ok(0) | Err(_) => break, // Ctrl-D, or stdin went away
            Ok(_) => {}
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Err(e) = execute(book, &line, &mut Printer) {
            print_chain(&e);
        }
    }
}
