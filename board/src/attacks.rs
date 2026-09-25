//! Attack tables for the pieces whose attacks depend only on their square (and colour, for pawns):
//! knight, king, pawn. Each table is built once, on first use, from its `build_*` function.
//! Pawn pushes and (later) sliders can be blocked, so they take the `occupied` squares instead.

use crate::bitboard::Bitboard;
use crate::color::Color;
use crate::masks::{file_mask, rank_mask};
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

/// `WHITE_PAWN_ATTACKS[sq]` = the squares a white pawn on `sq` captures on. Built on first use.
static WHITE_PAWN_ATTACKS: LazyLock<[Bitboard; 64]> = LazyLock::new(|| {
    let mut table = [Bitboard::EMPTY; 64];
    for sq in 0..64 {
        table[sq as usize] = build_pawn_attacks(Color::White, sq);
    }
    table
});

/// `BLACK_PAWN_ATTACKS[sq]` = the squares a black pawn on `sq` captures on. Built on first use.
static BLACK_PAWN_ATTACKS: LazyLock<[Bitboard; 64]> = LazyLock::new(|| {
    let mut table = [Bitboard::EMPTY; 64];
    for sq in 0..64 {
        table[sq as usize] = build_pawn_attacks(Color::Black, sq);
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

/// Computes the two diagonal squares a pawn of `color` on `sq` captures on (one on the edge files).
/// Used once to fill the pawn tables. Filled for every square, even ones a pawn can't stand on,
/// because the table is also read backwards: "which pawn squares attack my king?".
fn build_pawn_attacks(color: Color, sq: u8) -> Bitboard {
    let mut bb = Bitboard::EMPTY;
    let file = sq % 8;
    let rank = sq / 8;
    // nothing to attack beyond the last rank (white: rank 8, black: rank 1)
    if rank == 7 && color == Color::White || rank == 0 && color == Color::Black {
        return Bitboard::EMPTY;
    }
    if file == 0 && color == Color::White {
        bb.set(sq + 9);
        return bb;
    }
    if file == 7 && color == Color::White {
        bb.set(sq + 7);
        return bb;
    }
    if file == 0 && color == Color::Black {
        bb.set(sq - 7);
        return bb;
    }
    if file == 7 && color == Color::Black {
        bb.set(sq - 9);
        return bb;
    }
    bb.set((sq as i8 + 7 * color.forward()) as u8);
    bb.set((sq as i8 + 9 * color.forward()) as u8);
    bb
}

/// Every square a knight on `sq` attacks (empty board). One table lookup.
pub fn knight_attacks(sq: u8) -> Bitboard {
    KNIGHT_ATTACKS[sq as usize]
}

/// Every square a king on `sq` attacks (empty board). One table lookup.
pub fn king_attacks(sq: u8) -> Bitboard {
    KING_ATTACKS[sq as usize]
}

/// The squares a pawn of `color` on `sq` captures on. One table lookup.
/// Read backwards, `pawn_attacks(us, king_sq) & their_pawns` finds enemy pawns giving check.
pub fn pawn_attacks(color: Color, sq: u8) -> Bitboard {
    if color == Color::White {
        return WHITE_PAWN_ATTACKS[sq as usize];
    }
    BLACK_PAWN_ATTACKS[sq as usize]
}

/// Single pushes: every square the `pawns` of `color` can move one rank forward onto.
/// `occupied` = squares with any piece on them (either colour). All pawns are pushed at once; no table,
/// because the answer depends on what blocks them.
pub fn pawn_pushes(color: Color, pawns: Bitboard, occupied: Bitboard) -> Bitboard {
    match color {
        Color::White => (pawns << 8) & !occupied,
        Color::Black => (pawns >> 8) & !occupied,
    }
}

/// Double pushes: every square the `pawns` of `color` can reach by moving two ranks
/// forward from their starting rank. Both the square in between and the target must be empty.
pub fn pawn_double_pushes(color: Color, pawns: Bitboard, occupied: Bitboard) -> Bitboard {
    let single = pawn_pushes(color, pawns, occupied);
    // only pawns that just left their starting rank landed here
    let third_rank = match color {
        Color::White => rank_mask(2),
        Color::Black => rank_mask(5),
    };
    pawn_pushes(color, single & third_rank, occupied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::masks::rank_mask;

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

    // pawn attacks
    #[test]
    fn pawn_attacks_center() {
        assert_eq!(pawn_attacks(Color::White, 28), squares(&[35, 37])); // e4: d5 f5
        assert_eq!(pawn_attacks(Color::Black, 36), squares(&[27, 29])); // e5: d4 f4
    }
    #[test]
    fn white_pawn_a_file_does_not_wrap() {
        // a2: naive << 7 lands on h2
        assert_eq!(pawn_attacks(Color::White, 8), squares(&[17])); // b3
    }
    #[test]
    fn white_pawn_h_file_does_not_wrap() {
        // h2: naive << 9 lands on a4
        assert_eq!(pawn_attacks(Color::White, 15), squares(&[22])); // g3
    }
    #[test]
    fn black_pawn_edges_do_not_wrap() {
        assert_eq!(pawn_attacks(Color::Black, 48), squares(&[41])); // a7: b6
        assert_eq!(pawn_attacks(Color::Black, 55), squares(&[46])); // h7: g6
    }
    #[test]
    fn pawn_attacks_off_the_last_rank_are_empty() {
        for file in 0..8 {
            assert!(
                pawn_attacks(Color::White, 56 + file).is_empty(),
                "white file {file}"
            );
            assert!(
                pawn_attacks(Color::Black, file).is_empty(),
                "black file {file}"
            );
        }
    }
    #[test]
    fn pawn_attacks_are_symmetric_between_colors() {
        // a white pawn on `from` attacks `to` exactly when a black pawn on `to` attacks `from`
        for from in 0..64 {
            for to in pawn_attacks(Color::White, from) {
                assert!(
                    pawn_attacks(Color::Black, to).contains(from),
                    "{from} -> {to}"
                );
            }
            for to in pawn_attacks(Color::Black, from) {
                assert!(
                    pawn_attacks(Color::White, to).contains(from),
                    "{from} -> {to}"
                );
            }
        }
    }
    #[test]
    fn pawn_attack_totals_are_98() {
        // 7 ranks with room to move forward, 14 attacks each (6 middle files x 2 + 2 edge files x 1)
        let mut white = 0;
        let mut black = 0;
        for sq in 0..64 {
            white += pawn_attacks(Color::White, sq).count();
            black += pawn_attacks(Color::Black, sq).count();
        }
        assert_eq!(white, 98);
        assert_eq!(black, 98);
    }
    #[test]
    fn pawn_table_matches_builder() {
        for sq in 0..64 {
            for color in [Color::White, Color::Black] {
                assert_eq!(
                    pawn_attacks(color, sq),
                    build_pawn_attacks(color, sq),
                    "{color:?} square {sq}"
                );
            }
        }
    }

    // pawn pushes (set-wise: `occupied` = squares with any piece on them)
    #[test]
    fn white_single_push() {
        let pawns = squares(&[12]); // e2
        assert_eq!(pawn_pushes(Color::White, pawns, pawns), squares(&[20])); // e3
    }
    #[test]
    fn blocked_pawn_cannot_push_at_all() {
        let pawns = squares(&[12]); // e2
        let occupied = pawns | squares(&[20]); // piece on e3
        assert!(pawn_pushes(Color::White, pawns, occupied).is_empty());
        assert!(pawn_double_pushes(Color::White, pawns, occupied).is_empty());
    }
    #[test]
    fn white_double_push_from_rank_2() {
        let pawns = squares(&[12]); // e2
        assert_eq!(
            pawn_double_pushes(Color::White, pawns, pawns),
            squares(&[28])
        ); // e4
    }
    #[test]
    fn double_push_blocked_on_the_second_square() {
        let pawns = squares(&[12]); // e2
        let occupied = pawns | squares(&[28]); // piece on e4
        assert_eq!(pawn_pushes(Color::White, pawns, occupied), squares(&[20])); // e3 still fine
        assert!(pawn_double_pushes(Color::White, pawns, occupied).is_empty());
    }
    #[test]
    fn no_double_push_after_leaving_rank_2() {
        let pawns = squares(&[20]); // e3
        assert_eq!(pawn_pushes(Color::White, pawns, pawns), squares(&[28])); // e4
        assert!(pawn_double_pushes(Color::White, pawns, pawns).is_empty());
    }
    #[test]
    fn black_pushes_go_down() {
        let pawns = squares(&[52]); // e7
        assert_eq!(pawn_pushes(Color::Black, pawns, pawns), squares(&[44])); // e6
        assert_eq!(
            pawn_double_pushes(Color::Black, pawns, pawns),
            squares(&[36])
        ); // e5
    }
    #[test]
    fn all_white_pawns_push_at_once() {
        let pawns = rank_mask(1); // whole rank 2
        assert_eq!(pawn_pushes(Color::White, pawns, pawns), rank_mask(2));
        assert_eq!(pawn_double_pushes(Color::White, pawns, pawns), rank_mask(3));
    }
}
