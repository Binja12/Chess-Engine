//! Attack tables for the pieces whose moves depend only on their square (knight, king).
//! Each table is built once, on first use, from its `build_*` function.

use crate::bitboard::Bitboard;
use crate::masks::file_mask;
use std::sync::LazyLock;

/// How many files sideways a knight can move.
const KNIGHT_REACH: u8 = 2;
/// How many files sideways a king can move.
const KING_REACH: u8 = 1;

/// `KNIGHT_ATTACKS[sq]` = every square a knight on `sq` attacks. Built on first use.
static KNIGHT_ATTACKS: LazyLock<[Bitboard; 64]> = LazyLock::new(|| {
    let mut table = [Bitboard::EMPTY; 64];
    for sq in 0..64 {
        table[sq as usize] = build_knight_attacks(sq);
    }
    table
});

/// `KING_ATTACKS[sq]` = every square a king on `sq` attacks. Built on first use.
static KING_ATTACKS: LazyLock<[Bitboard; 64]> = LazyLock::new(|| {
    let mut table = [Bitboard::EMPTY; 64];
    for sq in 0..64 {
        table[sq as usize] = build_king_attacks(sq);
    }
    table
});

/// Computes every square a knight on `sq` attacks. Used once to fill `KNIGHT_ATTACKS`;
/// everywhere else call `knight_attacks`, which reads the table.
/// Shifts the knight by all 8 jumps, then keeps only files within `KNIGHT_REACH` of the knight:
/// a jump that wrapped around the board edge lands 6-7 files away.
fn build_knight_attacks(sq: u8) -> Bitboard {
    let mut bb = Bitboard::EMPTY;
    for jmp in [17, 15, 10, 6] {
        let mut tmp_down = Bitboard::EMPTY;
        tmp_down.set(sq);
        bb |= tmp_down >> jmp;
        let mut tmp_up = Bitboard::EMPTY;
        tmp_up.set(sq);
        bb |= tmp_up << jmp;
    }
    let mut mask = Bitboard::EMPTY;
    for reach in 0..=KNIGHT_REACH {
        if sq % 8 + reach <= 7 {
            mask |= file_mask(sq % 8 + reach);
        }
        if sq % 8 >= reach {
            mask |= file_mask(sq % 8 - reach);
        }
    }
    bb & mask
}

/// Computes every square a king on `sq` attacks. Used once to fill `KING_ATTACKS`;
/// everywhere else call `king_attacks`, which reads the table.
/// Shifts the king one step in all 8 directions, then keeps only its own file and the
/// files within `KING_REACH`: a step that wrapped around the board edge lands 7 files away.
fn build_king_attacks(sq: u8) -> Bitboard {
    let mut bb = Bitboard::EMPTY;
    for jmp in [1, 7, 8, 9] {
        let mut tmp_down = Bitboard::EMPTY;
        tmp_down.set(sq);
        bb |= tmp_down >> jmp;
        let mut tmp_up = Bitboard::EMPTY;
        tmp_up.set(sq);
        bb |= tmp_up << jmp;
    }
    let mut mask = file_mask(sq % 8); // own file, for the straight up/down steps
    if sq % 8 + KING_REACH <= 7 {
        mask |= file_mask(sq % 8 + KING_REACH);
    }
    if sq % 8 >= KING_REACH {
        mask |= file_mask(sq % 8 - KING_REACH);
    }
    bb & mask
}

/// Every square a knight on `sq` attacks (empty board). One table lookup.
pub fn knight_attacks(sq: u8) -> Bitboard {
    KNIGHT_ATTACKS[sq as usize]
}

/// Every square a king on `sq` attacks (empty board). One table lookup.
pub fn king_attacks(sq: u8) -> Bitboard {
    KING_ATTACKS[sq as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Builds a bitboard from a list of squares.
    fn squares(list: &[u8]) -> Bitboard {
        let mut bb = Bitboard::EMPTY;
        for &sq in list {
            bb.set(sq);
        }
        bb
    }
    #[test]
    fn knight_center_has_8() {
        assert_eq!(
            knight_attacks(27),
            squares(&[10, 12, 17, 21, 33, 37, 42, 44])
        )
    }
    #[test]
    fn knight_corners_have_2() {
        assert_eq!(knight_attacks(0), squares(&[10, 17]));
        assert_eq!(knight_attacks(63), squares(&[53, 46]))
    }
    #[test]
    fn knight_h_file_does_not_wrap() {
        // h1: naive +10 lands on b3, +17 on a4
        assert_eq!(knight_attacks(7), squares(&[13, 22]));
    }
    #[test]
    fn knight_g_file_does_not_wrap() {
        // g1: naive +10 (2 files right) lands on a3
        assert_eq!(knight_attacks(6), squares(&[12, 21, 23]));
    }
    #[test]
    fn knight_b_file_does_not_wrap() {
        // b1: naive +6 (2 files left) lands on h1
        assert_eq!(knight_attacks(1), squares(&[11, 16, 18]));
    }
    #[test]
    fn knight_attacks_are_symmetric() {
        for from in 0..64 {
            for to in knight_attacks(from) {
                assert!(knight_attacks(to).contains(from), "{from} -> {to}");
            }
        }
    }
    #[test]
    fn knight_total_is_336() {
        let mut total = 0;
        for sq in 0..64 {
            total += knight_attacks(sq).count();
        }
        assert_eq!(total, 336);
    }

    #[test]
    fn king_center_has_8() {
        // e4: d3 e3 f3, d4 f4, d5 e5 f5
        assert_eq!(king_attacks(28), squares(&[19, 20, 21, 27, 29, 35, 36, 37]));
    }
    #[test]
    fn king_corners_have_3() {
        assert_eq!(king_attacks(0), squares(&[1, 8, 9])); // a1: b1 a2 b2
        assert_eq!(king_attacks(63), squares(&[54, 55, 62])); // h8: g7 h7 g8
    }
    #[test]
    fn king_h_file_does_not_wrap() {
        // h4: naive +1 lands on a5
        assert_eq!(king_attacks(31), squares(&[22, 23, 30, 38, 39]));
    }
    #[test]
    fn king_a_file_does_not_wrap() {
        // a4: naive -1 lands on h3
        assert_eq!(king_attacks(24), squares(&[16, 17, 25, 32, 33]));
    }
    #[test]
    fn king_never_attacks_own_square() {
        for sq in 0..64 {
            assert!(!king_attacks(sq).contains(sq), "square {sq}");
        }
    }
    #[test]
    fn king_attacks_are_symmetric() {
        for from in 0..64 {
            for to in king_attacks(from) {
                assert!(king_attacks(to).contains(from), "{from} -> {to}");
            }
        }
    }
    #[test]
    fn king_total_is_420() {
        let mut total = 0;
        for sq in 0..64 {
            total += king_attacks(sq).count();
        }
        assert_eq!(total, 420);
    }

    #[test]
    fn knight_table_matches_builder() {
        for sq in 0..64 {
            assert_eq!(knight_attacks(sq), build_knight_attacks(sq), "square {sq}");
        }
    }
    #[test]
    fn king_table_matches_builder() {
        for sq in 0..64 {
            assert_eq!(king_attacks(sq), build_king_attacks(sq), "square {sq}");
        }
    }
}
