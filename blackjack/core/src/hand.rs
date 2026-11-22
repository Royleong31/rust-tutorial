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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Suit;

    fn cards(ranks: &[Rank]) -> Vec<Card> {
        ranks.iter().map(|&rank| Card { suit: Suit::Spades, rank }).collect()
    }

    fn hand(ranks: &[Rank]) -> Hand {
        Hand { cards: cards(ranks) }
    }

    /// Built directly (not via `new`) so tests can start from any number of cards.
    fn player(ranks: &[Rank], bet_amount: u64) -> PlayerHand {
        PlayerHand { bet_amount, hand: hand(ranks), hand_status: HandStatus::InProgress }
    }

    fn dealer(ranks: &[Rank]) -> DealerHand {
        DealerHand { hand: hand(ranks) }
    }

    fn settle(player_ranks: &[Rank], dealer_ranks: &[Rank], bet: u64) -> HandStatus {
        let mut p = player(player_ranks, bet);
        p.update_result(&dealer(dealer_ranks));
        p.status()
    }

    fn ended(outcome: Outcome, payout: u64) -> HandStatus {
        HandStatus::Ended { outcome, payout }
    }

    use Rank::*;

    // ---------- Hand::value ----------

    #[test]
    fn value_hard_totals() {
        assert_eq!(hand(&[Two, Three]).value(), 5);
        assert_eq!(hand(&[Ten, Seven]).value(), 17);
        assert_eq!(hand(&[King, Queen]).value(), 20);
        assert_eq!(hand(&[Ten, Five, Nine]).value(), 24);
    }

    #[test]
    fn value_soft_ace_counts_as_11() {
        assert_eq!(hand(&[Ace, Six]).value(), 17);
        assert_eq!(hand(&[Ace, King]).value(), 21);
    }

    #[test]
    fn value_ace_drops_to_1_to_avoid_bust() {
        assert_eq!(hand(&[Ace, Six, Ten]).value(), 17);
        assert_eq!(hand(&[Ace, Nine, Five]).value(), 15);
    }

    #[test]
    fn value_multiple_aces_only_one_counts_as_11() {
        assert_eq!(hand(&[Ace, Ace]).value(), 12);
        assert_eq!(hand(&[Ace, Ace, Nine]).value(), 21);
        assert_eq!(hand(&[Ace, Ace, Ace, Ace]).value(), 14);
        assert_eq!(hand(&[Ace, Ace, Ten]).value(), 12);
    }

    // ---------- blackjack / bust ----------

    #[test]
    fn blackjack_needs_exactly_two_cards() {
        assert!(hand(&[Ace, King]).is_blackjack());
        assert!(hand(&[Ten, Ace]).is_blackjack());
        assert!(!hand(&[Ace, Five, Five]).is_blackjack(), "three-card 21 is not blackjack");
        assert!(!hand(&[Ten, Queen]).is_blackjack());
    }

    #[test]
    fn bust_is_over_21() {
        assert!(hand(&[Ten, Five, Seven]).is_bust());
        assert!(!hand(&[Ten, Five, Six]).is_bust());
        assert!(!hand(&[Ace, Ten, Ten]).is_bust(), "ace drops to 1");
    }

    // ---------- PlayerHand::update_result ----------

    #[test]
    fn player_blackjack_pays_3_to_2() {
        assert_eq!(settle(&[Ace, King], &[Ten, Queen], 10), ended(Outcome::PlayerBlackjack, 25));
    }

    #[test]
    fn player_blackjack_odd_bet_rounds_down() {
        assert_eq!(settle(&[Ace, King], &[Ten, Nine], 5), ended(Outcome::PlayerBlackjack, 12));
    }

    #[test]
    fn player_blackjack_beats_dealer_three_card_21() {
        assert_eq!(settle(&[Ace, King], &[Seven, Seven, Seven], 10), ended(Outcome::PlayerBlackjack, 25));
    }

    #[test]
    fn both_blackjack_is_push() {
        assert_eq!(settle(&[Ace, King], &[Queen, Ace], 10), ended(Outcome::Push, 10));
    }

    #[test]
    fn dealer_blackjack_beats_player_20() {
        assert_eq!(settle(&[Ten, Queen], &[Ace, King], 10), ended(Outcome::DealerBlackjack, 0));
    }

    #[test]
    fn dealer_blackjack_beats_player_three_card_21() {
        assert_eq!(settle(&[Seven, Seven, Seven], &[Ace, King], 10), ended(Outcome::DealerBlackjack, 0));
    }

    #[test]
    fn player_bust_loses() {
        assert_eq!(settle(&[Ten, Five, Nine], &[Ten, Seven], 10), ended(Outcome::Lost, 0));
    }

    #[test]
    fn player_bust_loses_even_if_dealer_busts() {
        assert_eq!(settle(&[Ten, Five, Nine], &[Ten, Six, Nine], 10), ended(Outcome::Lost, 0));
    }

    #[test]
    fn dealer_bust_pays_1_to_1() {
        assert_eq!(settle(&[Ten, Two], &[Ten, Six, Nine], 10), ended(Outcome::Won, 20));
    }

    #[test]
    fn higher_total_wins() {
        assert_eq!(settle(&[Ten, Nine], &[Ten, Seven], 10), ended(Outcome::Won, 20));
    }

    #[test]
    fn lower_total_loses() {
        assert_eq!(settle(&[Ten, Seven], &[Ten, Nine], 10), ended(Outcome::Lost, 0));
    }

    #[test]
    fn equal_totals_push() {
        assert_eq!(settle(&[Ten, Eight], &[Nine, Nine], 10), ended(Outcome::Push, 10));
        assert_eq!(settle(&[Seven, Seven, Seven], &[Ten, Five, Six], 10), ended(Outcome::Push, 10));
    }

    #[test]
    fn payout_amount_after_settling() {
        let mut p = player(&[Ten, Nine], 10);
        p.update_result(&dealer(&[Ten, Seven]));
        assert_eq!(p.get_payout_amount(), 20);
    }

    #[test]
    #[should_panic(expected = "before hand ended")]
    fn payout_amount_before_settling_panics() {
        player(&[Ten, Nine], 10).get_payout_amount();
    }

    // ---------- DealerHand ----------

    #[test]
    fn upcard_and_hole_card_order() {
        let d = DealerHand::new(Card { suit: Suit::Hearts, rank: Nine }, Card { suit: Suit::Clubs, rank: Two });
        assert_eq!(d.upcard().rank, Nine);
        assert_eq!(d.hole_card().rank, Two);
    }

    #[test]
    fn dealer_stands_on_hard_17() {
        let mut d = dealer(&[Ten, Seven]);
        d.play(&mut Rng::new("unused".to_string(), 0, 51));
        assert_eq!(d.cards().len(), 2);
    }

    #[test]
    fn dealer_stands_on_soft_17() {
        let mut d = dealer(&[Ace, Six]);
        d.play(&mut Rng::new("unused".to_string(), 0, 51));
        assert_eq!(d.cards().len(), 2, "S17: dealer must not hit soft 17");
    }

    #[test]
    fn dealer_draws_until_at_least_17() {
        for i in 0..200 {
            let mut d = dealer(&[Two, Three]);
            d.play(&mut Rng::new(format!("dealer-{i}"), 0, 51));

            assert!(d.value() >= 17, "seed dealer-{i}: stopped at {}", d.value());
            assert!(d.cards().len() > 2);

            // It stopped at the first total of 17+: one card fewer was still under 17.
            let before_last = hand(&d.cards()[..d.cards().len() - 1].iter().map(|c| c.rank).collect::<Vec<_>>());
            assert!(before_last.value() < 17, "seed dealer-{i}: drew past 17");
        }
    }
}
