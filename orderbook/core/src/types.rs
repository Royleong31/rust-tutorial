//! Units: newtypes, so a price can't be passed where a quantity is expected.
//!
//! Prices are integers in the instrument's smallest unit (say cents). Never
//! f64: 0.1 + 0.2 != 0.3, and f64 isn't Ord, so it couldn't even be a
//! BTreeMap key.
//!
//! The fields are `pub`. These are plain values with no rules to protect,
//! and callers and tests build them all the time. Compare `Book`, whose
//! fields are private because they have to stay consistent with each other.

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Price(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Qty(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrderId(pub u64);

/// Who sent the order. Needed for self-trade prevention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraderId(pub u32);

impl Qty {
    pub const ZERO: Qty = Qty(0);
}

// Only the arithmetic that MEANS something is implemented:
//   Qty ± Qty -> Qty        Price - Price -> i64 (a distance)        Price × Qty -> i64 (notional)
// Price + Price would compile to nonsense, so it doesn't compile at all.
//
// Qty subtraction underflowing (taking more than there is) panics in debug
// builds. That's what we want: it can only happen through a bug in the book.
impl Add for Qty {
    type Output = Qty;
    fn add(self, rhs: Qty) -> Qty {
        Qty(self.0 + rhs.0)
    }
}

impl Sub for Qty {
    type Output = Qty;
    fn sub(self, rhs: Qty) -> Qty {
        Qty(self.0 - rhs.0)
    }
}

impl AddAssign for Qty {
    fn add_assign(&mut self, rhs: Qty) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Qty {
    fn sub_assign(&mut self, rhs: Qty) {
        self.0 -= rhs.0;
    }
}

impl Sum for Qty {
    fn sum<I: Iterator<Item = Qty>>(iter: I) -> Qty {
        iter.fold(Qty::ZERO, |a, b| a + b)
    }
}

impl Sub for Price {
    type Output = i64;
    fn sub(self, rhs: Price) -> i64 {
        self.0 - rhs.0
    }
}

impl Mul<Qty> for Price {
    type Output = i64;
    fn mul(self, rhs: Qty) -> i64 {
        self.0 * rhs.0 as i64
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // `pad` (instead of write!) makes width flags like {:>6} work.
        f.pad(&self.0.to_string())
    }
}

impl fmt::Display for Qty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&self.0.to_string())
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl fmt::Display for TraderId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}
