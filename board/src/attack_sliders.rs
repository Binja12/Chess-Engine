//! Sliding attacks (rook, bishop, queen) by walking each ray square by square.
//! Slow but obviously correct: the reference that magic bitboards are cross-tested against.
//! A ray stops at the board edge or at the first occupied square, which is included (a possible capture).

use crate::bitboard::Bitboard;

/// Every square a rook on `sq` attacks, given `occupied` (pieces of either colour).
/// The first blocker on each ray is included; `sq` itself being in `occupied` is ignored.
/// Each loop adds the square first, then stops if it was occupied.
pub fn rook_attacks_ray(sq: u8, occupied: Bitboard) -> Bitboard {
    let mut bb = Bitboard::EMPTY;
    let rank = sq as i8 / 8;
    //left: `tmp >= 0` first, because -1 / 8 == 0 would pass the rank check on rank 1
    let mut tmp = sq as i8 - 1;
    while tmp >= 0 && tmp / 8 == rank {
        bb.set(tmp as u8);
        if occupied.contains(tmp as u8) {
            break;
        }
        tmp -= 1;
    }
    //right
    tmp = sq as i8 + 1;
    while tmp / 8 == rank {
        bb.set(tmp as u8);
        if occupied.contains(tmp as u8) {
            break;
        }
        tmp += 1;
    }
    //up
    tmp = sq as i8 + 8;
    while tmp <= 63 {
        bb.set(tmp as u8);
        if occupied.contains(tmp as u8) {
            break;
        }
        tmp += 8;
    }
    //down
    tmp = sq as i8 - 8;
    while tmp >= 0 {
        bb.set(tmp as u8);
        if occupied.contains(tmp as u8) {
            break;
        }
        tmp -= 8;
    }
    bb
}

/// Every square a bishop on `sq` attacks, given `occupied` (pieces of either colour).
/// The first blocker on each ray is included; `sq` itself being in `occupied` is ignored.
/// Every diagonal step must move exactly one rank; a step that wrapped around the a/h edge
/// lands on the wrong rank, so checking the expected `rank` stops the ray at the edge.
pub fn bishop_attacks_ray(sq: u8, occupied: Bitboard) -> Bitboard {
    let mut bb = Bitboard::EMPTY;
    let mut rank = sq as i8 / 8 - 1;
    //left down
    let mut tmp = sq as i8 - 9;
    while tmp >= 0 && tmp / 8 == rank {
        bb.set(tmp as u8);
        if occupied.contains(tmp as u8) {
            break;
        }
        tmp -= 9;
        rank -= 1;
    }
    //right up
    tmp = sq as i8 + 9;
    rank = sq as i8 / 8 + 1;
    while tmp <= 63 && tmp / 8 == rank {
        bb.set(tmp as u8);
        if occupied.contains(tmp as u8) {
            break;
        }
        tmp += 9;
        rank += 1;
    }
    //left up
    tmp = sq as i8 + 7;
    rank = sq as i8 / 8 + 1;
    while tmp <= 63 && tmp / 8 == rank {
        bb.set(tmp as u8);
        if occupied.contains(tmp as u8) {
            break;
        }
        tmp += 7;
        rank += 1;
    }
    //right down
    tmp = sq as i8 - 7;
    rank = sq as i8 / 8 - 1;
    while tmp >= 0 && tmp / 8 == rank {
        bb.set(tmp as u8);
        if occupied.contains(tmp as u8) {
            break;
        }
        tmp -= 7;
        rank -= 1;
    }
    bb
}

/// Every square a queen on `sq` attacks: rook attacks plus bishop attacks.
pub fn queen_attacks_ray(sq: u8, occupied: Bitboard) -> Bitboard {
    bishop_attacks_ray(sq, occupied) | rook_attacks_ray(sq, occupied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attacks::king_attacks;
    use crate::masks::{
        FILE_A, FILE_H, MAIN_ANTI_DIAGONAL, MAIN_DIAGONAL, RANK_1, RANK_8, anti_diagonal_mask,
        diagonal_mask, file_mask, rank_mask,
    };

    /// Builds a bitboard from a list of squares.
    fn squares(list: &[u8]) -> Bitboard {
        let mut bb = Bitboard::EMPTY;
        for &sq in list {
            bb.set(sq);
        }
        bb
    }

    /// Every square occupied.
    const FULL: Bitboard = Bitboard { bits: u64::MAX };

    // 1-2. empty board: counts per square
    #[test]
    fn rook_empty_board_has_14_everywhere() {
        for sq in 0..64 {
            assert_eq!(
                rook_attacks_ray(sq, Bitboard::EMPTY).count(),
                14,
                "square {sq}"
            );
        }
    }
    #[test]
    fn rook_empty_board_is_file_and_rank() {
        // d4: the whole d-file and 4th rank, minus d4 itself
        let expected = (file_mask(3) | rank_mask(3)) ^ squares(&[27]);
        assert_eq!(rook_attacks_ray(27, Bitboard::EMPTY), expected);
    }
    #[test]
    fn bishop_empty_board_count_depends_on_ring() {
        // ring 0 = outer edge … ring 3 = the 4 centre squares: 7, 9, 11, 13 squares
        for sq in 0..64 {
            let (file, rank) = (sq % 8, sq / 8);
            let ring = file.min(7 - file).min(rank).min(7 - rank);
            let expected = 7 + 2 * ring as u32;
            assert_eq!(
                bishop_attacks_ray(sq, Bitboard::EMPTY).count(),
                expected,
                "square {sq}"
            );
        }
    }
    #[test]
    fn bishop_empty_board_is_both_diagonals() {
        let expected = (diagonal_mask(27) | anti_diagonal_mask(27)) ^ squares(&[27]);
        assert_eq!(bishop_attacks_ray(27, Bitboard::EMPTY), expected);
    }
    #[test]
    fn empty_board_totals() {
        let mut rook = 0;
        let mut bishop = 0;
        for sq in 0..64 {
            rook += rook_attacks_ray(sq, Bitboard::EMPTY).count();
            bishop += bishop_attacks_ray(sq, Bitboard::EMPTY).count();
        }
        assert_eq!(rook, 896);
        assert_eq!(bishop, 560);
    }

    // 3. corners: some rays are empty from the first step
    #[test]
    fn rook_corners() {
        assert_eq!(
            rook_attacks_ray(0, Bitboard::EMPTY),
            (FILE_A | RANK_1) ^ squares(&[0])
        );
        assert_eq!(
            rook_attacks_ray(63, Bitboard::EMPTY),
            (FILE_H | RANK_8) ^ squares(&[63])
        );
    }
    #[test]
    fn bishop_corners() {
        assert_eq!(
            bishop_attacks_ray(0, Bitboard::EMPTY),
            MAIN_DIAGONAL ^ squares(&[0])
        );
        assert_eq!(
            bishop_attacks_ray(63, Bitboard::EMPTY),
            MAIN_DIAGONAL ^ squares(&[63])
        );
        assert_eq!(
            bishop_attacks_ray(7, Bitboard::EMPTY),
            MAIN_ANTI_DIAGONAL ^ squares(&[7])
        );
        assert_eq!(
            bishop_attacks_ray(56, Bitboard::EMPTY),
            MAIN_ANTI_DIAGONAL ^ squares(&[56])
        );
    }

    // 4. no wrap across the a/h edge
    #[test]
    fn rook_does_not_wrap() {
        // h4 stepping right naively lands on a5; a4 stepping left lands on h3
        assert!(!rook_attacks_ray(31, Bitboard::EMPTY).contains(32));
        assert!(!rook_attacks_ray(24, Bitboard::EMPTY).contains(23));
    }
    #[test]
    fn bishop_does_not_wrap() {
        // h4: g5 f6 e7 d8, g3 f2 e1 and nothing on the a-file
        assert_eq!(
            bishop_attacks_ray(31, Bitboard::EMPTY),
            squares(&[38, 45, 52, 59, 22, 13, 4])
        );
        assert!((bishop_attacks_ray(31, Bitboard::EMPTY) & FILE_A).is_empty());
        assert!((bishop_attacks_ray(24, Bitboard::EMPTY) & FILE_H).is_empty());
    }

    // 5. blocker in the middle of a ray: included, nothing behind it
    #[test]
    fn rook_stops_at_blockers() {
        // d4 with pieces on d6 and f4
        let occupied = squares(&[43, 29]);
        let expected = squares(&[
            35, 43, // d5 d6
            28, 29, // e4 f4
            19, 11, 3, // d3 d2 d1
            26, 25, 24, // c4 b4 a4
        ]);
        assert_eq!(rook_attacks_ray(27, occupied), expected);
    }
    #[test]
    fn only_the_first_blocker_on_a_ray_counts() {
        // d4 with pieces on d6 and d8: d5 d6 attacked, d7 d8 hidden behind d6
        let attacks = rook_attacks_ray(27, squares(&[43, 59]));
        assert!(attacks.contains(35) && attacks.contains(43));
        assert!(!attacks.contains(51) && !attacks.contains(59));
    }
    #[test]
    fn bishop_stops_at_blockers() {
        // d4 with pieces on f6 and b2
        let occupied = squares(&[45, 9]);
        let expected = squares(&[
            36, 45, // e5 f6
            18, 9, // c3 b2
            34, 41, 48, // c5 b6 a7
            20, 13, 6, // e3 f2 g1
        ]);
        assert_eq!(bishop_attacks_ray(27, occupied), expected);
    }

    // 6. blocker on the first square of a ray
    #[test]
    fn adjacent_blockers_leave_one_square_per_ray() {
        assert_eq!(rook_attacks_ray(0, squares(&[8, 1])), squares(&[8, 1])); // a1: a2 b1
        assert_eq!(bishop_attacks_ray(0, squares(&[9])), squares(&[9])); // a1: b2
    }
    #[test]
    fn adjacent_blocker_cuts_only_its_own_ray() {
        // d4 with a piece on d5: north ray is just d5, the other three run to the edge
        let expected = (file_mask(3) | rank_mask(3)) ^ squares(&[27, 43, 51, 59]);
        assert_eq!(rook_attacks_ray(27, squares(&[35])), expected);
    }

    // 7. full board: only the neighbouring squares
    #[test]
    fn full_board_center() {
        assert_eq!(rook_attacks_ray(27, FULL), squares(&[35, 19, 28, 26])); // d5 d3 e4 c4
        assert_eq!(bishop_attacks_ray(27, FULL), squares(&[36, 34, 20, 18])); // e5 c5 e3 c3
    }
    #[test]
    fn full_board_rook_plus_bishop_is_king() {
        for sq in 0..64 {
            let rook = rook_attacks_ray(sq, FULL);
            let bishop = bishop_attacks_ray(sq, FULL);
            assert!((rook & bishop).is_empty(), "square {sq}");
            assert_eq!(rook | bishop, king_attacks(sq), "square {sq}");
        }
    }

    // 8. pieces off every ray change nothing
    #[test]
    fn off_ray_pieces_are_ignored() {
        // for the rook on d4, e5 c3 b6 are diagonal; for the bishop, d6 f4 are straight
        let empty_rook = rook_attacks_ray(27, Bitboard::EMPTY);
        let empty_bishop = bishop_attacks_ray(27, Bitboard::EMPTY);
        assert_eq!(rook_attacks_ray(27, squares(&[36, 18, 41])), empty_rook);
        assert_eq!(bishop_attacks_ray(27, squares(&[43, 29])), empty_bishop);
    }

    // 9. the slider's own square in `occupied` changes nothing
    #[test]
    fn own_square_is_ignored() {
        for sq in 0..64 {
            let own = squares(&[sq]);
            assert_eq!(
                rook_attacks_ray(sq, own),
                rook_attacks_ray(sq, Bitboard::EMPTY),
                "rook {sq}"
            );
            assert_eq!(
                bishop_attacks_ray(sq, own),
                bishop_attacks_ray(sq, Bitboard::EMPTY),
                "bishop {sq}"
            );
        }
        let blockers = squares(&[43, 29]);
        assert_eq!(
            rook_attacks_ray(27, blockers | squares(&[27])),
            rook_attacks_ray(27, blockers)
        );
    }

    // 10. queen = rook | bishop
    #[test]
    fn queen_is_rook_plus_bishop() {
        let occupancies = [
            Bitboard::EMPTY,
            FULL,
            Bitboard {
                bits: 0xFFFF_0000_0000_FFFF,
            }, // start position
            Bitboard {
                bits: 0x0000_1824_4200_0000,
            }, // a few centre pieces
            Bitboard {
                bits: 0x55AA_55AA_55AA_55AA,
            }, // checkerboard
        ];
        for occupied in occupancies {
            for sq in 0..64 {
                assert_eq!(
                    queen_attacks_ray(sq, occupied),
                    rook_attacks_ray(sq, occupied) | bishop_attacks_ray(sq, occupied),
                    "square {sq}, occupied {:#x}",
                    occupied.bits
                );
            }
        }
    }
}
