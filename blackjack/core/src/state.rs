use std::fmt;

use crate::card::Card;
use crate::deck::{CardSource, SeededDeck};
use crate::hand::{DealerHand, HandLike, PlayerHand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAction {
    Hit,
    Stand,
    // DoubleDown, Split,
    // BuyInsurance, RejectInsurance
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    Active,
    Ended { total_bet_amount: u64, total_payout: u64 },
}

pub struct BlackjackGameState {
    player_hand: PlayerHand,
    dealer_hand: DealerHand,
    deck: Box<dyn CardSource>,
    phase: GamePhase,
}

// Hand-written so `{:?}` follows the same visibility rules as the public API:
// only the dealer's upcard is shown while the game is active. The deck's own
// Debug hides the RNG seed.
impl fmt::Debug for BlackjackGameState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("BlackjackGameState");
        s.field("player_hand", &self.player_hand);

        match self.dealer_hand() {
            Some(dealer_hand) => s.field("dealer_hand", dealer_hand),
            None => s.field("dealer_upcard", self.dealer_upcard()),
        };

        s.field("deck", &self.deck)
            .field("phase", &self.phase)
            .finish()
    }
}

impl BlackjackGameState {
    pub fn new(seed: String, bet_amount: u64) -> BlackjackGameState {
        BlackjackGameState::with_deck(Box::new(SeededDeck::new(seed)), bet_amount)
    }

    /// The injection point: the game draws every card from `deck`.
    /// Cards are dealt player, player, dealer (upcard), dealer (hole card).
    pub(crate) fn with_deck(mut deck: Box<dyn CardSource>, bet_amount: u64) -> BlackjackGameState {
        let player_card_0 = deck.draw();
        let player_card_1 = deck.draw();
        let dealer_card_0 = deck.draw();
        let dealer_card_1 = deck.draw();

        let mut bj_state = BlackjackGameState {
            player_hand: PlayerHand::new(player_card_0, player_card_1, bet_amount),
            dealer_hand: DealerHand::new(dealer_card_0, dealer_card_1),
            deck,
            phase: GamePhase::Active,
        };

        if bj_state.dealer_hand.is_blackjack() || bj_state.player_hand.is_blackjack() {
            bj_state.end_game();
        }

        bj_state
    }

    pub fn player_action(&mut self, action: PlayerAction) {
        if self.is_over() {
            panic!("Game has already ended");
        }

        match action {
            PlayerAction::Hit => {
                self.player_hand.hit(self.deck.as_mut());
                let has_busted = self.player_hand.is_bust();

                if has_busted || self.player_hand.value() == 21 {
                    if !has_busted {
                        self.dealer_hand.play(self.deck.as_mut());
                    }

                    self.end_game();
                }
            }

            PlayerAction::Stand => {
                self.dealer_hand.play(self.deck.as_mut());
                self.end_game();
            }
        }
    }

    pub fn phase(&self) -> GamePhase {
        self.phase
    }

    pub fn is_over(&self) -> bool {
        matches!(self.phase, GamePhase::Ended { .. })
    }

    pub fn player_hand(&self) -> &PlayerHand {
        &self.player_hand
    }

    /// The dealer's face-up card. Always visible.
    pub fn dealer_upcard(&self) -> &Card {
        self.dealer_hand.upcard()
    }

    /// The dealer's full hand, including the hole card.
    /// `None` while the game is active so callers can't peek at the hole card.
    pub fn dealer_hand(&self) -> Option<&DealerHand> {
        if self.is_over() {
            Some(&self.dealer_hand)
        } else {
            None
        }
    }

    fn end_game(&mut self) {
        self.player_hand.update_result(&self.dealer_hand);
        self.phase = GamePhase::Ended {
            total_bet_amount: self.player_hand.bet_amount(),
            total_payout: self.player_hand.get_payout_amount(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Rank::{self, *};
    use crate::deck::ScriptedDeck;
    use crate::hand::{HandStatus, Outcome};

    /// Deal order: player, player, dealer upcard, dealer hole card, then every
    /// later draw (player hits first, then the dealer's draws).
    /// The scripted deck panics if the game draws more cards than listed.
    fn game(cards: &[Rank], bet: u64) -> BlackjackGameState {
        BlackjackGameState::with_deck(Box::new(ScriptedDeck::of(cards)), bet)
    }

    fn ranks(cards: &[Card]) -> Vec<Rank> {
        cards.iter().map(|c| c.rank).collect()
    }

    fn ended(outcome: Outcome, payout: u64) -> HandStatus {
        HandStatus::Ended { outcome, payout }
    }

    // ---------- the deal ----------

    #[test]
    fn deal_order_is_player_player_dealer_dealer() {
        let g = game(&[Two, Three, Nine, Seven], 10);

        assert_eq!(ranks(g.player_hand().cards()), vec![Two, Three]);
        assert_eq!(g.dealer_upcard().rank, Nine);
        assert!(!g.is_over());
    }

    #[test]
    fn player_blackjack_on_the_deal_ends_immediately() {
        let g = game(&[Ace, King, Ten, Seven], 10);

        assert!(g.is_over());
        assert_eq!(g.player_hand().status(), ended(Outcome::PlayerBlackjack, 25));
        assert_eq!(g.phase(), GamePhase::Ended { total_bet_amount: 10, total_payout: 25 });
    }

    #[test]
    fn dealer_blackjack_on_the_deal_ends_immediately() {
        let g = game(&[Ten, Nine, Ace, Queen], 10);

        assert!(g.is_over());
        assert_eq!(g.player_hand().status(), ended(Outcome::DealerBlackjack, 0));
    }

    #[test]
    fn both_blackjack_on_the_deal_is_a_push() {
        let g = game(&[Ace, King, Queen, Ace], 10);
        assert_eq!(g.player_hand().status(), ended(Outcome::Push, 10));
    }

    #[test]
    fn dealer_does_not_draw_when_the_deal_has_a_blackjack() {
        // Dealer shows 12 but must not play: the round is already decided.
        let g = game(&[Ace, King, Two, King], 10);
        assert_eq!(g.dealer_hand().unwrap().cards().len(), 2);
    }

    // ---------- stand ----------

    #[test]
    fn stand_makes_the_dealer_play() {
        // Regression: the dealer used to settle on their first two cards.
        // Dealer 5 → 10 → 20.
        let mut g = game(&[Ten, Eight, Two, Three, Five, King], 10);
        g.player_action(PlayerAction::Stand);

        let dealer = g.dealer_hand().unwrap();
        assert_eq!(ranks(dealer.cards()), vec![Two, Three, Five, King]);
        assert_eq!(g.player_hand().status(), ended(Outcome::Lost, 0));
    }

    #[test]
    fn stand_wins_when_dealer_busts() {
        // Dealer 16 → 26.
        let mut g = game(&[Ten, Two, Ten, Six, Queen], 10);
        g.player_action(PlayerAction::Stand);

        assert!(g.dealer_hand().unwrap().is_bust());
        assert_eq!(g.player_hand().status(), ended(Outcome::Won, 20));
    }

    #[test]
    fn stand_against_dealer_on_17_draws_nothing() {
        let mut g = game(&[Ten, Nine, Ten, Seven], 10);
        g.player_action(PlayerAction::Stand);

        assert_eq!(g.phase(), GamePhase::Ended { total_bet_amount: 10, total_payout: 20 });
    }

    // ---------- hit ----------

    #[test]
    fn hit_below_21_keeps_the_game_going() {
        let mut g = game(&[Two, Three, Ten, Seven, Four], 10);
        g.player_action(PlayerAction::Hit);

        assert!(!g.is_over());
        assert_eq!(g.player_hand().value(), 9);
        assert_eq!(g.player_hand().status(), HandStatus::InProgress);
        assert!(g.dealer_hand().is_none());
    }

    #[test]
    fn hit_and_bust_ends_without_the_dealer_drawing() {
        // Dealer shows 5 and would have to draw, but the deck has no cards for it.
        let mut g = game(&[Ten, Six, Two, Three, Nine], 10);
        g.player_action(PlayerAction::Hit);

        assert!(g.player_hand().is_bust());
        assert_eq!(g.player_hand().status(), ended(Outcome::Lost, 0));
        assert_eq!(g.dealer_hand().unwrap().cards().len(), 2);
    }

    #[test]
    fn hit_to_21_auto_stands_and_dealer_plays() {
        //                player     dealer      hit   dealer draws: 5 → 15 → 19
        let mut g = game(&[Five, Six, Two, Three, King, Ten, Four], 10);
        g.player_action(PlayerAction::Hit);

        assert!(g.is_over(), "should auto-stand on 21");
        assert_eq!(g.player_hand().value(), 21);
        assert_eq!(g.dealer_hand().unwrap().value(), 19);
        assert_eq!(g.player_hand().status(), ended(Outcome::Won, 20));
    }

    #[test]
    fn several_hits_then_stand() {
        // Player 4 → 9 → 15, stands. Dealer 17, no draw. Dealer wins.
        let mut g = game(&[Two, Two, Ten, Seven, Five, Six], 10);
        g.player_action(PlayerAction::Hit);
        g.player_action(PlayerAction::Hit);
        g.player_action(PlayerAction::Stand);

        assert_eq!(g.player_hand().value(), 15);
        assert_eq!(g.player_hand().status(), ended(Outcome::Lost, 0));
    }

    // ---------- game over ----------

    #[test]
    #[should_panic(expected = "already ended")]
    fn acting_after_the_game_ends_panics() {
        let mut g = game(&[Ten, Nine, Ten, Seven], 10);
        g.player_action(PlayerAction::Stand);
        g.player_action(PlayerAction::Hit);
    }

    #[test]
    #[should_panic(expected = "already ended")]
    fn acting_after_a_dealt_blackjack_panics() {
        let mut g = game(&[Ace, King, Ten, Seven], 10);
        g.player_action(PlayerAction::Stand);
    }

    #[test]
    fn push_returns_the_bet() {
        let mut g = game(&[Ten, Seven, Ten, Seven], 30);
        g.player_action(PlayerAction::Stand);

        assert_eq!(g.phase(), GamePhase::Ended { total_bet_amount: 30, total_payout: 30 });
        assert_eq!(g.player_hand().status(), ended(Outcome::Push, 30));
    }

    // ---------- visibility ----------

    #[test]
    fn dealer_hand_hidden_until_game_ends() {
        let mut g = game(&[Ten, Eight, Ten, Seven], 10);
        assert!(g.dealer_hand().is_none());
        assert_eq!(g.dealer_upcard().rank, Ten);

        g.player_action(PlayerAction::Stand);
        assert_eq!(g.dealer_hand().unwrap().hole_card().rank, Seven);
    }

    #[test]
    fn debug_hides_hole_card_while_active() {
        let mut g = game(&[Ten, Eight, Ten, Seven], 10);

        let active = format!("{g:?}");
        assert!(active.contains("dealer_upcard"), "{active}");
        assert!(!active.contains("dealer_hand"), "{active}");

        g.player_action(PlayerAction::Stand);
        let ended = format!("{g:?}");
        assert!(ended.contains("dealer_hand"), "{ended}");
    }

    #[test]
    fn debug_hides_the_seed_in_production() {
        let g = BlackjackGameState::new("very-secret-seed".to_string(), 10);
        let debug = format!("{g:?}");
        assert!(!debug.contains("very-secret-seed"), "{debug}");
    }
}
