use std::io::{self, Write};
use std::process;

use blackjack::{BlackjackGameState, Card, HandLike, HandStatus, Outcome, PlayerAction};

const DEFAULT_BALANCE: u64 = 1000;

const USAGE: &str = "\
Usage: blackjack-cli [--seed <SEED>] [--balance <AMOUNT>]

  --seed <SEED>        Base seed for the session (replays the same deals).
                       Defaults to a random seed.
  --balance <AMOUNT>   Starting balance. Defaults to 1000.
  -h, --help           Show this help.";

struct Args {
    seed: Option<String>,
    balance: u64,
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            process::exit(2);
        }
    };

    let base_seed = args.seed.unwrap_or_else(random_seed);
    let mut balance = args.balance;
    let mut round: u64 = 0;

    println!("Blackjack — dealer stands on soft 17, blackjack pays 3:2.\n");

    loop {
        if balance == 0 {
            println!("You're out of money.");
            break;
        }

        let Some(bet) = prompt_bet(balance) else { break };
        balance -= bet;
        round += 1;

        let mut game = BlackjackGameState::new(format!("{base_seed}-r{round}"), bet);

        while !game.is_over() {
            render(&game);
            let Some(action) = prompt_action() else {
                println!("\nQuitting mid-round; the bet of {bet} is forfeited.");
                return finish(balance, &base_seed);
            };
            game.player_action(action);
        }

        render(&game);
        let payout = report_result(&game);
        balance = balance.saturating_add(payout);
        println!();
    }

    finish(balance, &base_seed);
}

fn finish(balance: u64, base_seed: &str) {
    println!("Final balance: {balance}");
    // Shown only at the end: the base seed predicts every round, so revealing it mid-session
    // would let you see future cards.
    println!("Replay this session with: --seed {base_seed}");
}

// ---------- args ----------

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let mut seed = None;
    let mut balance = DEFAULT_BALANCE;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" => seed = Some(args.next().ok_or("--seed needs a value")?),
            "--balance" => {
                let value = args.next().ok_or("--balance needs a value")?;
                balance = value
                    .parse()
                    .map_err(|_| format!("invalid balance: {value}"))?;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Args { seed, balance })
}

fn random_seed() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("OS random number generator failed");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------- input ----------

/// Prints `prompt` and reads one trimmed line. `None` on EOF (Ctrl-D) or read error.
fn read_line(prompt: &str) -> Option<String> {
    print!("{prompt}");
    io::stdout().flush().ok()?;

    let mut line = String::new();
    match io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}

/// `None` means the player wants to stop.
fn prompt_bet(balance: u64) -> Option<u64> {
    loop {
        let input = read_line(&format!("Balance: {balance}   Bet (or q to quit): "))?;

        if input.eq_ignore_ascii_case("q") {
            return None;
        }

        match input.parse::<u64>() {
            Ok(bet) if (1..=balance).contains(&bet) => return Some(bet),
            Ok(_) => println!("  Bet must be between 1 and {balance}."),
            Err(_) => println!("  Enter a whole number, or q to quit."),
        }
    }
}

/// `None` on EOF.
fn prompt_action() -> Option<PlayerAction> {
    loop {
        let input = read_line("  [h]it or [s]tand? ")?;

        match input.to_ascii_lowercase().as_str() {
            "h" | "hit" => return Some(PlayerAction::Hit),
            "s" | "stand" => return Some(PlayerAction::Stand),
            _ => println!("  Type h or s."),
        }
    }
}

// ---------- output ----------

fn hand_str(cards: &[Card]) -> String {
    cards
        .iter()
        .map(|card| card.to_string())
        .collect::<Vec<_>>()
        .join("  ")
}

fn render(game: &BlackjackGameState) {
    println!();

    match game.dealer_hand() {
        None => println!("  Dealer:  {}  ??", game.dealer_upcard()),
        Some(dealer) => print_hand("Dealer:", dealer),
    }

    print_hand("You:", game.player_hand());
    println!();
}

fn print_hand(label: &str, hand: &impl HandLike) {
    println!("  {label:<8} {}   ({})", hand_str(hand.cards()), hand.value());
}

/// Prints the outcome and returns the payout to credit back to the balance.
fn report_result(game: &BlackjackGameState) -> u64 {
    let HandStatus::Ended { outcome, payout } = game.player_hand().status() else {
        unreachable!("report_result is only called after the game is over");
    };

    let message = match outcome {
        Outcome::PlayerBlackjack => "Blackjack! You win 3:2.",
        Outcome::DealerBlackjack => "Dealer has blackjack. You lose.",
        Outcome::Won => "You win!",
        Outcome::Push => "Push. Bet returned.",
        Outcome::Lost if game.player_hand().is_bust() => "Bust. You lose.",
        Outcome::Lost => "Dealer wins.",
    };

    println!("  {message}  Payout: {payout}");
    payout
}
