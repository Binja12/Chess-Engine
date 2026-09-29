//! Castling data, shared by move generation (`castling_moves`) and by `make_move` /
//! `unmake_move` / `from_fen` in `position.rs`: for each color and side, the right it needs and
//! every square it looks at. The home squares of kings and rooks are written down only here;
//! everything else reads them from `CASTLES`.

use crate::bitboard::Bitboard;
use crate::color::Color;
use crate::moves::{KING_CASTLE, QUEEN_CASTLE};
use crate::position::{BLACK_KINGSIDE, BLACK_QUEENSIDE, WHITE_KINGSIDE, WHITE_QUEENSIDE};
use std::sync::LazyLock;

// the squares castling looks at (a1 = 0 … h8 = 63)
const A1: u8 = 0;
const B1: u8 = 1;
const C1: u8 = 2;
const D1: u8 = 3;
const E1: u8 = 4;
const F1: u8 = 5;
const G1: u8 = 6;
const H1: u8 = 7;
const A8: u8 = 56;
const B8: u8 = 57;
const C8: u8 = 58;
const D8: u8 = 59;
const E8: u8 = 60;
const F8: u8 = 61;
const G8: u8 = 62;
const H8: u8 = 63;

/// All four castling rights.
const ALL_RIGHTS: u8 = WHITE_KINGSIDE | WHITE_QUEENSIDE | BLACK_KINGSIDE | BLACK_QUEENSIDE;

/// One way to castle for one color, with every square it looks at spelled out.
pub(crate) struct Castle {
    /// The castling right it needs, e.g. `WHITE_KINGSIDE`.
    pub(crate) right: u8,
    /// The squares between king and rook: all must be empty.
    pub(crate) between: Bitboard,
    /// Where the king starts (e1 / e8): it must not be in check.
    pub(crate) king_from: u8,
    /// The square the king passes over: it must not be attacked. The rook lands here.
    pub(crate) king_crosses: u8,
    /// Where the king lands: it must not be attacked.
    pub(crate) king_to: u8,
    /// Where the rook starts (h1 / a1 / h8 / a8).
    pub(crate) rook_from: u8,
    /// `KING_CASTLE` or `QUEEN_CASTLE`.
    pub(crate) flag: u8,
}

/// `CASTLES[color as usize][side]`, side 0 = king side (short), 1 = queen side (long).
/// On the queen side b1 / b8 must be empty (the rook crosses it) but may be attacked (the king
/// does not). A `static`: one table in memory, handed out by reference.
pub(crate) static CASTLES: [[Castle; 2]; 2] = [
    // White
    [
        Castle {
            right: WHITE_KINGSIDE,
            // `1 << sq` is the bit of one square; `|` joins them into a set
            between: Bitboard {
                bits: 1 << F1 | 1 << G1,
            },
            king_from: E1,
            king_crosses: F1,
            king_to: G1,
            rook_from: H1,
            flag: KING_CASTLE,
        },
        Castle {
            right: WHITE_QUEENSIDE,
            between: Bitboard {
                bits: 1 << B1 | 1 << C1 | 1 << D1,
            },
            king_from: E1,
            king_crosses: D1,
            king_to: C1,
            rook_from: A1,
            flag: QUEEN_CASTLE,
        },
    ],
    // Black
    [
        Castle {
            right: BLACK_KINGSIDE,
            between: Bitboard {
                bits: 1 << F8 | 1 << G8,
            },
            king_from: E8,
            king_crosses: F8,
            king_to: G8,
            rook_from: H8,
            flag: KING_CASTLE,
        },
        Castle {
            right: BLACK_QUEENSIDE,
            between: Bitboard {
                bits: 1 << B8 | 1 << C8 | 1 << D8,
            },
            king_from: E8,
            king_crosses: D8,
            king_to: C8,
            rook_from: A8,
            flag: QUEEN_CASTLE,
        },
    ],
];

/// The castling entry for `color` and a castling move's `flag` (`KING_CASTLE` or
/// `QUEEN_CASTLE`).
pub(crate) fn castle(color: Color, flag: u8) -> &'static Castle {
    let side = if flag == KING_CASTLE { 0 } else { 1 };
    &CASTLES[color as usize][side]
}

/// `RIGHTS_KEPT[sq]`: the rights that survive a move from or to `sq`. Built on first use from
/// `CASTLES`: a right ends when its king's or its rook's home square is touched.
static RIGHTS_KEPT: LazyLock<[u8; 64]> = LazyLock::new(|| {
    let mut kept = [ALL_RIGHTS; 64];
    for castle in CASTLES.iter().flatten() {
        kept[castle.king_from as usize] &= !castle.right;
        kept[castle.rook_from as usize] &= !castle.right;
    }
    kept
});

/// The castling rights that survive a move from or to `sq`: moving away from, or capturing on,
/// a king's or rook's home square ends the rights that need that piece. One table lookup.
pub(crate) fn rights_kept(sq: u8) -> u8 {
    RIGHTS_KEPT[sq as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1. the table ends exactly the right rights, and only on the six home squares
    #[test]
    fn rights_kept_follows_the_home_squares() {
        assert_eq!(rights_kept(E1), BLACK_KINGSIDE | BLACK_QUEENSIDE);
        assert_eq!(rights_kept(H1), ALL_RIGHTS & !WHITE_KINGSIDE);
        assert_eq!(rights_kept(A1), ALL_RIGHTS & !WHITE_QUEENSIDE);
        assert_eq!(rights_kept(E8), WHITE_KINGSIDE | WHITE_QUEENSIDE);
        assert_eq!(rights_kept(H8), ALL_RIGHTS & !BLACK_KINGSIDE);
        assert_eq!(rights_kept(A8), ALL_RIGHTS & !BLACK_QUEENSIDE);
        for sq in 0..64 {
            if ![E1, H1, A1, E8, H8, A8].contains(&sq) {
                assert_eq!(rights_kept(sq), ALL_RIGHTS, "square {sq}");
            }
        }
    }

    // 2. `castle` finds the entry of each color and flag
    #[test]
    fn castle_finds_each_entry() {
        assert_eq!(castle(Color::White, KING_CASTLE).king_to, G1);
        assert_eq!(castle(Color::White, QUEEN_CASTLE).king_to, C1);
        assert_eq!(castle(Color::Black, KING_CASTLE).king_to, G8);
        assert_eq!(castle(Color::Black, QUEEN_CASTLE).king_to, C8);
    }
}
