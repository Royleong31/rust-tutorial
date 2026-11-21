use crate::card::{Card, Rank};
use crate::hand::HandStatus::IN_PROGRESS;
use crate::rng::Rng;

pub struct Hand {
    cards: Vec<Card>,
}

impl Hand {
    pub fn draw(&mut self, rng: &mut Rng) -> &Vec<Card> {
        let card = Card::draw(rng);
        self.cards.push(card);
        return &self.cards;
    }

    pub fn value(&self) -> u8 {
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

    pub fn is_blackjack(&self) -> bool {
        let hand_value = self.value();

        self.number_of_cards_in_hand() == 2 && hand_value == 21
    }

    pub fn is_bust(&self) -> bool {
        self.value() > 21
    }

    pub fn number_of_cards_in_hand(&self) -> usize {
        self.cards.iter().len()
    }
}

pub enum Outcome {
    PLAYER_BLACKJACK, DEALER_BLACKJACK, WON, PUSH, LOST
}

pub enum HandStatus {
    IN_PROGRESS,
    ENDED { outcome: Outcome, payout: u64 }
}

pub struct PlayerHand {
    pub bet_amount: u64,
    hand: Hand,
    pub hand_status: HandStatus
}

impl PlayerHand {
    pub fn cards(&self) -> &[Card] {
        &self.hand.cards
    }

    // pub fn can_double(&self) -> bool {
    //     self.hand.number_of_cards_in_hand() == 2
    // }

    pub fn hand(&self) -> &Hand {
        &self.hand
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

    pub fn hit(&mut self, rng: &mut Rng) {
        self.hand.draw(rng);
    }

    pub fn new(card_0: Card, card_1: Card, bet_amount: u64) -> PlayerHand {
        let mut player_cards: Vec<Card> = Vec::new();
        player_cards.push(card_0);
        player_cards.push(card_1);

        PlayerHand {
            bet_amount: bet_amount,
            hand: Hand {
                cards: player_cards,
            },
            hand_status: IN_PROGRESS
        }
    }
    
    pub(crate) fn update_result(&mut self, dealer_hand: &DealerHand) {
        let player_blackjack = self.is_blackjack();
        let dealer_blackjack = dealer_hand.is_blackjack();

        if player_blackjack && !dealer_blackjack {
            self.hand_status = HandStatus::ENDED { outcome: Outcome::PLAYER_BLACKJACK, payout: self.bet_amount * 5 / 2 };
            return;
        }

        if dealer_blackjack && !player_blackjack {
            self.hand_status = HandStatus::ENDED { outcome: Outcome::DEALER_BLACKJACK, payout: 0 };
            return;
        }

        let hand_lost = self.is_bust() || (!dealer_hand.is_bust() && dealer_hand.value() > self.value());

        if hand_lost {
            self.hand_status = HandStatus::ENDED { outcome: Outcome::LOST, payout: 0 };
            return;
        }

        if dealer_hand.is_bust() || dealer_hand.value() < self.value() {
            self.hand_status = HandStatus::ENDED { outcome: Outcome::WON, payout: self.bet_amount * 2 };
            return;
        }

        self.hand_status = HandStatus::ENDED { outcome: Outcome::PUSH, payout: self.bet_amount }
    }
    
    pub(crate) fn get_payout_amount(&self) -> u64 {
        match self.hand_status {
            HandStatus::ENDED { payout, .. } => payout,
            HandStatus::IN_PROGRESS => panic!("get_payout_amount called before hand ended"),
        }
    }
}

pub struct DealerHand {
    hand: Hand,
}

impl DealerHand {
    pub fn hand(&self) -> &Hand {
        &self.hand
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

    pub fn play(&mut self, rng: &mut Rng) -> &[Card] {
        // Dealer draws until 17 or more, and stands on soft 17 (S17 rule).
        while self.hand.value() < 17 {
            self.hand.draw(rng);
        }

        &self.hand.cards
    }

    pub fn new(up_card: Card, hole_card: Card) -> DealerHand {
        let mut dealer_cards: Vec<Card> = Vec::new();
        dealer_cards.push(up_card);
        dealer_cards.push(hole_card);

        DealerHand {
            hand: Hand {
                cards: dealer_cards,
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
