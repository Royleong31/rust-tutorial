use crate::card::Card;
use crate::hand::{DealerHand, PlayerHand};
use crate::rng::Rng;

const MIN_CARD_RNG_VALUE: u64 = 0;
const MAX_CARD_RNG_VALUE: u64 = 51;

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

#[derive(Debug)]
pub struct BlackjackGameState {
    player_hand: PlayerHand,
    dealer_hand: DealerHand,
    rng: Rng,
    phase: GamePhase,
}

impl BlackjackGameState {
    pub fn new(seed: String, bet_amount: u64) -> BlackjackGameState {
        let mut rng = Rng::new(seed, MIN_CARD_RNG_VALUE, MAX_CARD_RNG_VALUE);
        let player_card_0 = Card::draw(&mut rng);
        let player_card_1 = Card::draw(&mut rng);
        let dealer_card_0 = Card::draw(&mut rng);
        let dealer_card_1 = Card::draw(&mut rng);

        let mut bj_state = BlackjackGameState {
            player_hand: PlayerHand::new(player_card_0, player_card_1, bet_amount),
            dealer_hand: DealerHand::new(dealer_card_0, dealer_card_1),
            rng,
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
                self.player_hand.hit(&mut self.rng);
                let has_busted = self.player_hand.is_bust();

                if has_busted || self.player_hand.value() == 21 {
                    if !has_busted {
                        self.dealer_hand.play(&mut self.rng);
                    }

                    self.end_game();
                }
            }

            PlayerAction::Stand => {
                self.dealer_hand.play(&mut self.rng);
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
