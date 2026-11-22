use crate::card::{Card, Rank};
use crate::rng::Rng;

#[derive(Debug)]
pub(crate) struct Hand {
    cards: Vec<Card>,
}

impl Hand {
    pub(crate) fn draw(&mut self, rng: &mut Rng) {
        let card = Card::draw(rng);
        self.cards.push(card);
    }

    pub(crate) fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub(crate) fn value(&self) -> u8 {
        let mut hand_value = self
            .cards
            .iter()
            .fold(0u8, |acc, card| acc + card.rank.value());
        let has_ace = self.cards.iter().any(|card| card.rank == Rank::Ace);

        if has_ace && hand_value + 10 <= 21 {
            hand_value += 10;
        }

        hand_value
    }

    pub(crate) fn is_blackjack(&self) -> bool {
        let hand_value = self.value();

        self.number_of_cards_in_hand() == 2 && hand_value == 21
    }

    pub(crate) fn is_bust(&self) -> bool {
        self.value() > 21
    }

    pub(crate) fn number_of_cards_in_hand(&self) -> usize {
        self.cards.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    PlayerBlackjack,
    DealerBlackjack,
    Won,
    Push,
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandStatus {
    InProgress,
    Ended { outcome: Outcome, payout: u64 },
}

#[derive(Debug)]
pub struct PlayerHand {
    bet_amount: u64,
    hand: Hand,
    hand_status: HandStatus,
}

impl PlayerHand {
    pub fn cards(&self) -> &[Card] {
        self.hand.cards()
    }

    // pub fn can_double(&self) -> bool {
    //     self.hand.number_of_cards_in_hand() == 2
    // }

    pub fn bet_amount(&self) -> u64 {
        self.bet_amount
    }

    pub fn status(&self) -> HandStatus {
        self.hand_status
    }

    pub fn is_bust(&self) -> bool {
        self.hand.is_bust()
    }

    pub fn is_blackjack(&self) -> bool {
        self.hand.is_blackjack()
    }

    // pub fn can_split(&self) -> bool {
    //     let first_card = self
    //         .hand
    //         .cards
    //         .get(0)
    //         .expect("dealer hand always has 2 cards");
    //     let second_card = self
    //         .hand
    //         .cards
    //         .get(1)
    //         .expect("dealer hand always has 2 cards");

    //     self.hand.number_of_cards_in_hand() == 2 && first_card.rank == second_card.rank
    // }

    pub fn value(&self) -> u8 {
        self.hand.value()
    }

    pub(crate) fn hit(&mut self, rng: &mut Rng) {
        self.hand.draw(rng);
    }

    pub(crate) fn new(card_0: Card, card_1: Card, bet_amount: u64) -> PlayerHand {
        PlayerHand {
            bet_amount,
            hand: Hand {
                cards: vec![card_0, card_1],
            },
            hand_status: HandStatus::InProgress,
        }
    }

    pub(crate) fn update_result(&mut self, dealer_hand: &DealerHand) {
        let player_blackjack = self.is_blackjack();
        let dealer_blackjack = dealer_hand.is_blackjack();

        if player_blackjack && !dealer_blackjack {
            // 3:2 payout. Integer division rounds odd bets down (bet 5 pays 12).
            self.hand_status = HandStatus::Ended { outcome: Outcome::PlayerBlackjack, payout: self.bet_amount * 5 / 2 };
            return;
        }

        if dealer_blackjack && !player_blackjack {
            self.hand_status = HandStatus::Ended { outcome: Outcome::DealerBlackjack, payout: 0 };
            return;
        }

        let hand_lost = self.is_bust() || (!dealer_hand.is_bust() && dealer_hand.value() > self.value());

        if hand_lost {
            self.hand_status = HandStatus::Ended { outcome: Outcome::Lost, payout: 0 };
            return;
        }

        if dealer_hand.is_bust() || dealer_hand.value() < self.value() {
            self.hand_status = HandStatus::Ended { outcome: Outcome::Won, payout: self.bet_amount * 2 };
            return;
        }

        self.hand_status = HandStatus::Ended { outcome: Outcome::Push, payout: self.bet_amount }
    }

    pub(crate) fn get_payout_amount(&self) -> u64 {
        match self.hand_status {
            HandStatus::Ended { payout, .. } => payout,
            HandStatus::InProgress => panic!("get_payout_amount called before hand ended"),
        }
    }
}

#[derive(Debug)]
pub struct DealerHand {
    hand: Hand,
}

impl DealerHand {
    pub fn cards(&self) -> &[Card] {
        self.hand.cards()
    }

    pub fn is_blackjack(&self) -> bool {
        self.hand.is_blackjack()
    }

    pub fn upcard(&self) -> &Card {
        self.hand
            .cards
            .get(0)
            .expect("dealer hand always has 2 cards")
    }

    pub fn hole_card(&self) -> &Card {
        self.hand
            .cards
            .get(1)
            .expect("dealer hand always has 2 cards")
    }

    pub(crate) fn play(&mut self, rng: &mut Rng) {
        // Dealer draws until 17 or more, and stands on soft 17 (S17 rule).
        while self.hand.value() < 17 {
            self.hand.draw(rng);
        }
    }

    pub(crate) fn new(up_card: Card, hole_card: Card) -> DealerHand {
        DealerHand {
            hand: Hand {
                cards: vec![up_card, hole_card],
            },
        }
    }

    pub fn value(&self) -> u8 {
        self.hand.value()
    }

    pub fn is_bust(&self) -> bool {
        self.hand.is_bust()
    }
}
