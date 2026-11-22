use std::fmt;

use crate::card::Card;
use crate::rng::Rng;

const MIN_CARD_RNG_VALUE: u64 = 0;
const MAX_CARD_RNG_VALUE: u64 = 51;

/// Where the game gets its cards from. The game state only depends on this
/// trait, so tests can inject a deck that deals a fixed sequence.
///
/// `Debug` is a supertrait so `Box<dyn CardSource>` can be printed.
pub(crate) trait CardSource: fmt::Debug {
    fn draw(&mut self) -> Card;
}

/// Production source: every card is drawn independently from the seeded RNG
/// (an infinite deck, so duplicates are possible).
#[derive(Debug)]
pub(crate) struct SeededDeck {
    rng: Rng,
}

impl SeededDeck {
    pub(crate) fn new(seed: String) -> SeededDeck {
        SeededDeck {
            rng: Rng::new(seed, MIN_CARD_RNG_VALUE, MAX_CARD_RNG_VALUE),
        }
    }
}

impl CardSource for SeededDeck {
    fn draw(&mut self) -> Card {
        Card::draw(&mut self.rng)
    }
}

/// Test source: deals the given cards in order, and panics if the code under
/// test draws more cards than the test scripted.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ScriptedDeck {
    cards: std::collections::VecDeque<Card>,
}

#[cfg(test)]
impl ScriptedDeck {
    /// All cards are spades; tests only care about ranks.
    pub(crate) fn of(ranks: &[crate::card::Rank]) -> ScriptedDeck {
        ScriptedDeck {
            cards: ranks
                .iter()
                .map(|&rank| Card { suit: crate::card::Suit::Spades, rank })
                .collect(),
        }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.cards.len()
    }
}

#[cfg(test)]
impl CardSource for ScriptedDeck {
    fn draw(&mut self) -> Card {
        self.cards
            .pop_front()
            .expect("scripted deck ran out: the code drew more cards than the test expected")
    }
}
