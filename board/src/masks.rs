//! Precomputed masks: bitboards for groups of squares (files, ranks, diagonals).
//! Squares, files and ranks are 0-based: a1 = 0 … h8 = 63, file a = 0 … h = 7,
//! rank 1 = 0 … rank 8 = 7, so `sq = rank * 8 + file`.

use crate::bitboard::Bitboard;

/// The a-file: a1 … a8.
pub const FILE_A: Bitboard = Bitboard {
    bits: 0x0101_0101_0101_0101,
};
/// The h-file: h1 … h8.
pub const FILE_H: Bitboard = Bitboard {
    bits: 0x8080_8080_8080_8080,
};
/// The long a1 → h8 diagonal. Every other `/` diagonal is this one shifted up or down.
pub const MAIN_DIAGONAL: Bitboard = Bitboard {
    bits: 0x8040_2010_0804_0201,
};
/// The long h1 → a8 anti-diagonal. Every other `\` diagonal is this one shifted up or down.
pub const MAIN_ANTI_DIAGONAL: Bitboard = Bitboard {
    bits: 0x0102_0408_1020_4080,
};
/// Rank 1: a1 … h1.
pub const RANK_1: Bitboard = Bitboard {
    bits: 0x0000_0000_0000_00FF,
};
/// Rank 8: a8 … h8.
pub const RANK_8: Bitboard = Bitboard {
    bits: 0xFF00_0000_0000_0000,
};

/// All 8 squares of `file` (0 = a … 7 = h).
pub fn file_mask(file: u8) -> Bitboard {
    FILE_A << file
}
/// All 8 squares of `rank` (0 = rank 1 … 7 = rank 8).
pub fn rank_mask(rank: u8) -> Bitboard {
    RANK_1 << (rank * 8)
}
/// The `/` diagonal (a1 → h8 direction) through `sq`, including `sq` itself.
/// Shifts the main diagonal by whole ranks, so it never wraps across the board edge.
pub fn diagonal_mask(sq: u8) -> Bitboard {
    let d = (sq / 8) as i8 - (sq % 8) as i8; // rank - file, can be negative
    if d >= 0 {
        MAIN_DIAGONAL << (8 * d) as u8 // move up d ranks
    } else {
        MAIN_DIAGONAL >> (-8 * d) as u8 // move down -d ranks
    }
}
/// The `\` anti-diagonal (h1 → a8 direction) through `sq`, including `sq` itself.
/// Shifts the main anti-diagonal by whole ranks, so it never wraps across the board edge.
pub fn anti_diagonal_mask(sq: u8) -> Bitboard {
    let d = (sq / 8) as i8 + (sq % 8) as i8 - 7; // rank + file - 7
    if d >= 0 {
        MAIN_ANTI_DIAGONAL << (8 * d) as u8
    } else {
        MAIN_ANTI_DIAGONAL >> (-8 * d) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // files & ranks
    fn file_a_and_h_constants() {
        let mut file_a = Bitboard::EMPTY;
        for rank in 0..8 {
            file_a.set(rank * 8);
        }
        assert_eq!(file_a, FILE_A);
        let mut file_h = Bitboard::EMPTY;
        for rank in 0..8 {
            file_h.set(rank * 8 + 7);
        }
        assert_eq!(file_h, FILE_H);
    }
    #[test]
    fn rank_1_and_8_constants() {
        let mut rank_1 = Bitboard::EMPTY;
        for file in 0..8 {
            rank_1.set(file);
        }
        assert_eq!(rank_1, RANK_1);
        let mut rank_8 = Bitboard::EMPTY;
        const RANK_8_START: u8 = 56;
        for file in 0..8 {
            rank_8.set(RANK_8_START + file);
        }
        assert_eq!(rank_8, RANK_8);
    }
    #[test]
    fn file_mask_edges_match_constants() {
        assert_eq!(file_mask(0), FILE_A);
        assert_eq!(file_mask(7), FILE_H);
    }
    #[test]
    fn rank_mask_edges_match_constants() {
        assert_eq!(rank_mask(0), RANK_1);
        assert_eq!(rank_mask(7), RANK_8);
    }
    #[test]
    fn files_cover_board_without_overlap() {
        let mut all = Bitboard::EMPTY;
        for f in 0..8 {
            all |= file_mask(f);
        }
        assert_eq!(all, !Bitboard::EMPTY); // all 64 bits set
        for i in 0..8 {
            for j in 0..8 {
                if i != j {
                    assert!(
                        (file_mask(i) & file_mask(j)).is_empty(),
                        "files {i} and {j}"
                    );
                }
            }
        }
    }
    #[test]
    fn ranks_cover_board_without_overlap() {
        let mut all = Bitboard::EMPTY;
        for r in 0..8 {
            all |= rank_mask(r);
        }
        assert_eq!(all, !Bitboard::EMPTY); // all 64 bits set
        for i in 0..8 {
            for j in 0..8 {
                if i != j {
                    assert!(
                        (rank_mask(i) & rank_mask(j)).is_empty(),
                        "ranks {i} and {j}"
                    );
                }
            }
        }
    }
    #[test]
    fn file_and_rank_cross_at_one_square() {
        for r in 0..8 {
            for f in 0..8 {
                assert_eq!(
                    file_mask(f) & rank_mask(r),
                    Bitboard::from_square(r * 8 + f),
                    "rank {r}, file {f}"
                );
            }
        }
    }

    // diagonal (/)
    #[test]
    fn diagonal_long_a1_h8() {
        let mut expected = Bitboard::EMPTY;
        for i in 0..8 {
            expected.set(i * 9); // a1, b2, c3 ... h8
        }
        assert_eq!(diagonal_mask(0), expected);
    }
    #[test]
    fn diagonal_short_corners() {
        assert_eq!(diagonal_mask(7), Bitboard::from_square(7)); // h1
        assert_eq!(diagonal_mask(56), Bitboard::from_square(56)); // a8
    }
    #[test]
    fn diagonal_through_e4() {
        let mut expected = Bitboard::EMPTY;
        for i in 0..7 {
            expected.set(1 + i * 9); // b1, c2, d3, e4, f5, g6, h7
        }
        assert_eq!(diagonal_mask(28), expected);
    }
    #[test]
    fn diagonal_does_not_wrap_from_h_file() {
        let g1_h2 = Bitboard::from_square(6) | Bitboard::from_square(15);
        assert_eq!(diagonal_mask(15), g1_h2);
    }

    // anti-diagonal (\)
    #[test]
    fn anti_diagonal_long_h1_a8() {
        let mut expected = Bitboard::EMPTY;
        for i in 1..=8 {
            expected.set(i * 7); // h1, g2, f3 ... a8
        }
        assert_eq!(anti_diagonal_mask(7), expected);
    }
    #[test]
    fn anti_diagonal_short_corners() {
        assert_eq!(anti_diagonal_mask(0), Bitboard::from_square(0)); // a1
        assert_eq!(anti_diagonal_mask(63), Bitboard::from_square(63)); // h8
    }
    #[test]
    fn anti_diagonal_does_not_wrap_from_a_file() {
        let a2_b1 = Bitboard::from_square(8) | Bitboard::from_square(1);
        assert_eq!(anti_diagonal_mask(8), a2_b1);
    }

    // both
    #[test]
    fn diagonals_cross_only_at_their_square() {
        for sq in 0..64 {
            assert_eq!(
                diagonal_mask(sq) & anti_diagonal_mask(sq),
                Bitboard::from_square(sq),
                "square {sq}"
            );
        }
    }
}
