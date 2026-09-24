//! `Bitboard`: a set of squares stored as one `u64`, one bit per square.
//! Bit 0 = a1, bit 7 = h1, bit 56 = a8, bit 63 = h8 (`sq = rank * 8 + file`).

/// A set of board squares: bit `sq` is 1 when square `sq` is in the set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bitboard {
    pub bits: u64,
}
impl Bitboard {
    /// The empty set: no squares.
    pub const EMPTY: Bitboard = Bitboard { bits: 0 };
    /// A bitboard holding only `sq`.
    pub fn from_square(sq: u8) -> Bitboard {
        Bitboard { bits: 1u64 << sq }
    }
    /// True if `sq` is in the set.
    pub fn contains(self, sq: u8) -> bool {
        self.bits & (1u64 << sq) != 0
    }
    /// Adds `sq` to the set. Setting it twice is the same as once.
    pub fn set(&mut self, sq: u8) {
        self.bits |= 1u64 << sq;
    }
    /// Removes `sq` from the set. Does nothing if it is not there.
    pub fn clear(&mut self, sq: u8) {
        self.bits &= !(1u64 << sq);
    }
    /// Number of squares in the set.
    pub fn count(self) -> u32 {
        self.bits.count_ones()
    }
    /// True if the set has no squares.
    pub fn is_empty(self) -> bool {
        self == Bitboard::EMPTY
    }
    /// The lowest square in the set. Must not be called on an empty bitboard (returns 64).
    pub fn lsb(self) -> u8 {
        self.bits.trailing_zeros() as u8
    }
    /// Removes the lowest square from the set and returns it. Must not be called on an empty bitboard.
    pub fn pop_lsb(&mut self) -> u8 {
        let tmp = self.lsb();
        self.clear(tmp);
        tmp
    }
}
/// Iterates the squares from lowest (a1) to highest (h8), emptying the bitboard as it goes.
/// Iterate over a copy (`for sq in bb`) to keep the original.
impl Iterator for Bitboard {
    type Item = u8;
    /// The next lowest square, or `None` when no squares are left.
    fn next(&mut self) -> Option<u8> {
        match self.is_empty() {
            true => None,
            false => Some(self.pop_lsb()),
        }
    }
}
/// Prints the board as 8 lines of `X` (in the set) and `.` (not in the set),
/// rank 8 on top and the a-file on the left, as seen from White's side.
impl std::fmt::Display for Bitboard {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        const SIZE: u8 = 8;
        for rank in (0..SIZE).rev() {
            for file in 0..SIZE {
                if self.contains(rank * SIZE + file) {
                    write!(f, "X")?;
                } else {
                    write!(f, ".")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
/// `a & b`: squares in both sets (intersection).
impl std::ops::BitAnd for Bitboard {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Bitboard {
            bits: self.bits & rhs.bits,
        }
    }
}
/// `a | b`: squares in either set (union).
impl std::ops::BitOr for Bitboard {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Bitboard {
            bits: self.bits | rhs.bits,
        }
    }
}
/// `!a`: every square not in the set (complement).
impl std::ops::Not for Bitboard {
    type Output = Self;
    fn not(self) -> Self::Output {
        Bitboard { bits: !self.bits }
    }
}
/// `a ^ b`: squares in exactly one of the two sets.
impl std::ops::BitXor for Bitboard {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self::Output {
        Bitboard {
            bits: self.bits ^ rhs.bits,
        }
    }
}
/// `a &= b`: same as `a = a & b`.
impl std::ops::BitAndAssign for Bitboard {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}
/// `a |= b`: same as `a = a | b`.
impl std::ops::BitOrAssign for Bitboard {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}
/// `a ^= b`: same as `a = a ^ b`.
impl std::ops::BitXorAssign for Bitboard {
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}
/// `a << n`: moves every square up by `n` bits (`<< 8` = one rank up).
/// Squares pushed past h8 are dropped. Sideways shifts wrap from the h-file
/// to the next rank's a-file; mask with `FILE_H` first to prevent that.
impl std::ops::Shl<u8> for Bitboard {
    type Output = Self;
    fn shl(self, rhs: u8) -> Self::Output {
        Bitboard {
            bits: self.bits << rhs,
        }
    }
}
/// `a >> n`: moves every square down by `n` bits (`>> 8` = one rank down).
/// Squares pushed below a1 are dropped. Sideways shifts wrap from the a-file
/// to the previous rank's h-file; mask with `FILE_A` first to prevent that.
impl std::ops::Shr<u8> for Bitboard {
    type Output = Self;
    fn shr(self, rhs: u8) -> Self::Output {
        Bitboard {
            bits: self.bits >> rhs,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_square_sets_only_that_bit() {
        assert_eq!(Bitboard::from_square(10).bits, 1024);
        assert_eq!(Bitboard::from_square(0).bits, 1);
        assert_eq!(Bitboard::from_square(63).bits, 1u64 << 63);
    }
    #[test]
    fn contains_reads_the_right_bit() {
        let bb = Bitboard { bits: 0b1000 };
        assert!(!bb.contains(2));
        assert!(!bb.contains(4));
        assert!(bb.contains(3));
    }
    #[test]
    fn set_adds_squares() {
        let mut bb = Bitboard::EMPTY;
        bb.set(0);
        bb.set(63);
        assert_eq!(bb.bits, 1 | 1u64 << 63);
    }

    #[test]
    fn set_twice_is_same_as_once() {
        let mut bb = Bitboard::EMPTY;
        bb.set(5);
        bb.set(5);
        assert_eq!(bb, Bitboard::from_square(5));
    }

    #[test]
    fn clear_removes_only_that_square() {
        let mut bb = Bitboard { bits: 0b1001 };
        bb.clear(3);
        assert_eq!(bb.bits, 0b0001);
    }

    #[test]
    fn clear_on_empty_square_does_nothing() {
        let mut bb = Bitboard::from_square(5);
        bb.clear(6);
        assert_eq!(bb, Bitboard::from_square(5));
    }

    #[test]
    fn count_counts_set_bits() {
        assert_eq!(Bitboard::EMPTY.count(), 0);
        assert_eq!(Bitboard { bits: 0b1011 }.count(), 3);
        assert_eq!(Bitboard { bits: u64::MAX }.count(), 64);
    }

    #[test]
    fn is_empty_works() {
        assert!(Bitboard::EMPTY.is_empty());
        assert!(!Bitboard::from_square(0).is_empty());
        assert!(!Bitboard::from_square(63).is_empty());
    }

    #[test]
    fn lsb_finds_lowest_set_bit() {
        assert_eq!(Bitboard { bits: 0b1000 }.lsb(), 3);
        assert_eq!(Bitboard { bits: 0b1010 }.lsb(), 1);
        assert_eq!(Bitboard { bits: 1u64 << 63 }.lsb(), 63);
    }

    #[test]
    fn pop_lsb_returns_and_removes_lowest() {
        let mut bb = Bitboard { bits: 0b1010 };
        assert_eq!(bb.pop_lsb(), 1);
        assert_eq!(bb.bits, 0b1000);
        assert_eq!(bb.pop_lsb(), 3);
        assert!(bb.is_empty());
    }

    #[test]
    fn iterates_squares_low_to_high() {
        let squares: Vec<u8> = Bitboard { bits: 0b1010_0001 }.collect();
        assert_eq!(squares, vec![0, 5, 7]);
    }

    #[test]
    fn iterating_empty_yields_nothing() {
        let squares: Vec<u8> = Bitboard::EMPTY.collect();
        assert_eq!(squares, Vec::<u8>::new());
    }

    #[test]
    fn iteration_visits_count_squares() {
        let bb = Bitboard { bits: 0xFF00 };
        let squares: Vec<u8> = bb.collect();
        assert_eq!(squares.len(), bb.count() as usize);
    }

    #[test]
    fn display_prints_board_rank8_on_top() {
        let mut bb = Bitboard::EMPTY;
        bb.set(28); // e4
        bb.set(53); // f7
        let expected = "\
........\n\
.....X..\n\
........\n\
........\n\
....X...\n\
........\n\
........\n\
........\n";
        assert_eq!(format!("{}", bb), expected);
    }

    #[test]
    fn display_prints_board_all_cornors() {
        let mut bb = Bitboard::EMPTY;
        bb.set(0);
        bb.set(7);
        bb.set(56);
        bb.set(63);
        let expected = "\
        X......X\n\
        ........\n\
        ........\n\
        ........\n\
        ........\n\
        ........\n\
        ........\n\
        X......X\n";
        assert_eq!(format!("{}", bb), expected)
    }

    #[test]
    fn and_keeps_common_squares() {
        let a = Bitboard { bits: 0b1100 };
        let b = Bitboard { bits: 0b1010 };
        assert_eq!(a & b, Bitboard { bits: 0b1000 });
    }

    #[test]
    fn or_merges_squares() {
        let a = Bitboard { bits: 0b1100 };
        let b = Bitboard { bits: 0b1010 };
        assert_eq!(a | b, Bitboard { bits: 0b1110 });
    }

    #[test]
    fn not_flips_all_64_bits() {
        assert_eq!(!Bitboard::EMPTY, Bitboard { bits: u64::MAX });
        assert_eq!(!Bitboard { bits: u64::MAX }, Bitboard::EMPTY);
    }

    #[test]
    fn empty_is_identity_for_or_and_zero_for_and() {
        let a = Bitboard { bits: 0b1011 };
        assert_eq!(a & Bitboard::EMPTY, Bitboard::EMPTY);
        assert_eq!(a | Bitboard::EMPTY, a);
    }

    #[test]
    fn and_not_removes_squares() {
        let a = Bitboard { bits: 0b1111 };
        let b = Bitboard { bits: 0b0101 };
        assert_eq!(a & !b, Bitboard { bits: 0b1010 });
    }

    #[test]
    fn xor_keeps_squares_in_exactly_one() {
        let a = Bitboard { bits: 0b1100 };
        let b = Bitboard { bits: 0b1010 };
        assert_eq!(a ^ b, Bitboard { bits: 0b0110 });
        assert_eq!(a ^ a, Bitboard::EMPTY);
    }

    #[test]
    fn xor_twice_restores_original() {
        // make/unmake relies on this: toggling the same squares twice is a no-op
        let a = Bitboard { bits: 0b1011_0001 };
        let mv = Bitboard { bits: 0b0001_0001 };
        assert_eq!((a ^ mv) ^ mv, a);
    }

    #[test]
    fn assign_operators_match_plain_ones() {
        let a = Bitboard { bits: 0b1100 };
        let b = Bitboard { bits: 0b1010 };
        let mut x = a;
        x &= b;
        assert_eq!(x, a & b);
        let mut y = a;
        y |= b;
        assert_eq!(y, a | b);
        let mut z = a;
        z ^= b;
        assert_eq!(z, a ^ b);
    }

    #[test]
    fn shift_left_8_moves_up_one_rank() {
        assert_eq!(Bitboard::from_square(0) << 8, Bitboard::from_square(8)); // a1 -> a2
        assert_eq!(Bitboard { bits: 0xFF } << 8, Bitboard { bits: 0xFF00 }); // rank 1 -> rank 2
    }

    #[test]
    fn shift_right_8_moves_down_one_rank() {
        assert_eq!(Bitboard::from_square(8) >> 8, Bitboard::from_square(0)); // a2 -> a1
    }

    #[test]
    fn shift_by_zero_changes_nothing() {
        let a = Bitboard { bits: 0b1011 };
        assert_eq!(a << 0, a);
        assert_eq!(a >> 0, a);
    }

    #[test]
    fn shifting_off_the_board_drops_squares() {
        assert_eq!(Bitboard::from_square(56) << 8, Bitboard::EMPTY); // a8 up: gone, no wrap to rank 1
        assert_eq!(Bitboard::from_square(3) >> 8, Bitboard::EMPTY); // d1 down: gone
    }

    #[test]
    fn shift_left_1_wraps_h_file_to_next_rank() {
        // Documents a trap: shifts know nothing about files, so h1 + 1 = a2.
        // The file masks in CE-63 exist to cut these squares off.
        assert_eq!(Bitboard::from_square(7) << 1, Bitboard::from_square(8));
    }
}
