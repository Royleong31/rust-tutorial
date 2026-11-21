use crate::card::Card;
use crate::hand::{DealerHand, PlayerHand};
use crate::rng::Rng;

const MIN_CARD_RNG_VALUE: u64 = 0;
const MAX_CARD_RNG_VALUE: u64 = 51;

pub enum PlayerAction {
    Hit,
    Stand,
    // DoubleDown, Split,
    // BuyInsurance, RejectInsurance
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
  PlayerWins, PlayerLoses, Push
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    Active,
    Ended { total_bet_amount: u64, total_payout: u64}
}

pub struct BlackjackGameState {
    player_hand: PlayerHand,
    dealer_hand: DealerHand,
    rng: Rng,
    phase: GamePhase,
}

impl BlackjackGameState {
    pub fn player_action(&mut self, action: PlayerAction) {
        if self.phase != GamePhase::Active {
            panic!("Game has already ended");
        }

        match action {
            PlayerAction::Hit => {
                self.player_hand.hit(&mut self.rng);
                let has_busted = self.player_hand.hand().is_bust();

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

    fn end_game(&mut self) {
        self.player_hand.update_result(&self.dealer_hand);
        self.phase = GamePhase::Ended {total_bet_amount: self.player_hand.bet_amount, total_payout: self.player_hand.get_payout_amount() };
    }

    pub fn new(seed: String, bet_amount: u64) -> BlackjackGameState {
        let mut rng = Rng::new(seed, MIN_CARD_RNG_VALUE, MAX_CARD_RNG_VALUE);
        let player_card_0 = Card::draw(&mut rng);
        let player_card_1 = Card::draw(&mut rng);
        let dealer_card_0 = Card::draw(&mut rng);
        let dealer_card_1 = Card::draw(&mut rng);

        let mut bj_state = BlackjackGameState {
            player_hand: PlayerHand::new(player_card_0, player_card_1, bet_amount),
            dealer_hand: DealerHand::new(dealer_card_0, dealer_card_1),
            rng: rng,
            phase: GamePhase::Active,
        };

        if bj_state.dealer_hand.is_blackjack() || bj_state.player_hand.is_blackjack() {
            bj_state.end_game();
        }

        bj_state
    }
}
