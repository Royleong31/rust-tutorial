use std::fmt;

use crate::rng::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}
impl Card {
    pub(crate) fn draw(rng: &mut Rng) -> Card {
        let random_number: usize = rng
            .get_random_number()
            .try_into()
            .expect("expect u64 random number to be convertible to usize");
        let quotient: usize = random_number / 13;
        let remainder = random_number % 13;

        Card {
            suit: Suit::ALL[quotient],
            rank: Rank::ALL[remainder],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suit {
    Diamonds,
    Clubs,
    Hearts,
    Spades,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    Ace,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Diamonds, Suit::Clubs, Suit::Hearts, Suit::Spades];
}

impl Rank {
    pub fn value(self) -> u8 {
        match self {
            Rank::Ace => 1,
            Rank::Two => 2,
            Rank::Three => 3,
            Rank::Four => 4,
            Rank::Five => 5,
            Rank::Six => 6,
            Rank::Seven => 7,
            Rank::Eight => 8,
            Rank::Nine => 9,
            Rank::Ten => 10,
            Rank::Jack => 10,
            Rank::Queen => 10,
            Rank::King => 10,
        }
    }

    pub const ALL: [Rank; 13] = [
        Rank::Ace,
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Ten,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
    ];
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = match self {
            Suit::Diamonds => "♦",
            Suit::Clubs => "♣",
            Suit::Hearts => "♥",
            Suit::Spades => "♠",
        };
        f.write_str(symbol)
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Rank::Ace => "A",
            Rank::Two => "2",
            Rank::Three => "3",
            Rank::Four => "4",
            Rank::Five => "5",
            Rank::Six => "6",
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Jack => "J",
            Rank::Queen => "Q",
            Rank::King => "K",
        };
        f.write_str(label)
    }
}

/// Prints like `A♠` or `10♥`.
impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank, self.suit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_values() {
        assert_eq!(Rank::Ace.value(), 1);
        assert_eq!(Rank::Two.value(), 2);
        assert_eq!(Rank::Nine.value(), 9);
        for face in [Rank::Ten, Rank::Jack, Rank::Queen, Rank::King] {
            assert_eq!(face.value(), 10, "{face:?}");
        }
    }

    #[test]
    fn display() {
        let card = |rank, suit| Card { suit, rank };
        assert_eq!(card(Rank::Ace, Suit::Spades).to_string(), "A♠");
        assert_eq!(card(Rank::Ten, Suit::Hearts).to_string(), "10♥");
        assert_eq!(card(Rank::King, Suit::Diamonds).to_string(), "K♦");
        assert_eq!(card(Rank::Seven, Suit::Clubs).to_string(), "7♣");
    }

    #[test]
    fn draw_covers_all_52_cards() {
        let mut rng = Rng::new("card-coverage".to_string(), 0, 51);
        let mut seen: Vec<Card> = Vec::new();

        for _ in 0..2_000 {
            let card = Card::draw(&mut rng);
            if !seen.contains(&card) {
                seen.push(card);
            }
        }

        assert_eq!(seen.len(), 52);
    }
}
