//! Integration tests: these use only the public API, the same way the CLI does.
//! The detailed rules are covered by the unit tests inside src/.

use blackjack::{BlackjackGameState, GamePhase, HandStatus, Outcome, PlayerAction};

const BET: u64 = 10;

fn new_game(seed: &str) -> BlackjackGameState {
    BlackjackGameState::new(seed.to_string(), BET)
}

/// The first seed ("seed-0", "seed-1", ...) whose fresh game matches `condition`.
fn find_seed(condition: impl Fn(&BlackjackGameState) -> bool) -> String {
    (0..10_000)
        .map(|i| format!("seed-{i}"))
        .find(|seed| condition(&new_game(seed)))
        .expect("no matching seed found")
}

/// A simple strategy: hit below 17, then stand.
fn play_out(game: &mut BlackjackGameState) {
    while !game.is_over() {
        if game.player_hand().value() < 17 {
            game.player_action(PlayerAction::Hit);
        } else {
            game.player_action(PlayerAction::Stand);
        }
    }
}

#[test]
fn same_seed_deals_the_same_game() {
    let mut a = new_game("replay");
    let mut b = new_game("replay");
    play_out(&mut a);
    play_out(&mut b);

    assert_eq!(a.player_hand().cards(), b.player_hand().cards());
    assert_eq!(a.dealer_hand().unwrap().cards(), b.dealer_hand().unwrap().cards());
    assert_eq!(a.phase(), b.phase());
}

#[test]
fn dealer_hand_is_hidden_until_the_game_ends() {
    let mut game = new_game(&find_seed(|g| !g.is_over()));
    assert!(game.dealer_hand().is_none());

    game.player_action(PlayerAction::Stand);
    assert!(game.dealer_hand().is_some());
}

#[test]
fn a_dealt_blackjack_ends_the_game_immediately() {
    let game = new_game(&find_seed(|g| g.player_hand().is_blackjack()));

    assert!(game.is_over());
    // Either 3:2 or, if the dealer also has blackjack, a push.
    let HandStatus::Ended { outcome, .. } = game.player_hand().status() else {
        panic!("game should be over");
    };
    assert!(matches!(outcome, Outcome::PlayerBlackjack | Outcome::Push), "{outcome:?}");
}

#[test]
fn payout_matches_the_outcome_over_many_games() {
    for i in 0..500 {
        let mut game = new_game(&format!("many-{i}"));
        play_out(&mut game);

        let HandStatus::Ended { outcome, payout } = game.player_hand().status() else {
            panic!("game should be over");
        };
        let expected = match outcome {
            Outcome::PlayerBlackjack => BET * 5 / 2,
            Outcome::Won => BET * 2,
            Outcome::Push => BET,
            Outcome::Lost | Outcome::DealerBlackjack => 0,
        };

        assert_eq!(payout, expected, "many-{i}: {outcome:?}");
        assert_eq!(game.phase(), GamePhase::Ended { total_bet_amount: BET, total_payout: payout });
    }
}

#[test]
#[should_panic(expected = "already ended")]
fn acting_after_the_game_ends_panics() {
    let mut game = new_game("over");
    play_out(&mut game);
    game.player_action(PlayerAction::Hit);
}
