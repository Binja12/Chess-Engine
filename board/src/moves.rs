//! `Move`: one move packed into 16 bits, and `MoveList`: a fixed-size list of moves.
//!
//! Layout of a move (`bits`):
//! - bits 0-5: from square (0-63)
//! - bits 6-11: to square (0-63)
//! - bits 12-15: flag, one of the flag constants below
//!
//! The flag numbers are chosen so single bits carry meaning: flag bit 2 (value 4) means capture
//! and flag bit 3 (value 8) means promotion; for promotions the low 2 bits pick the piece
//! (0 = knight, 1 = bishop, 2 = rook, 3 = queen). Anything the flag does not say (which piece
//! moves, which piece is captured) is read from the board.

use crate::color::Color;
use crate::piece::{Piece, PieceKind};
use crate::square::square_name;

/// A normal move: no capture, nothing special.
pub const QUIET: u8 = 0;
/// A pawn moving two squares; sets the en passant square.
pub const DOUBLE_PAWN_PUSH: u8 = 1;
/// King-side castling (the king's move, e.g. e1g1); the rook moves too.
pub const KING_CASTLE: u8 = 2;
/// Queen-side castling (the king's move, e.g. e1c1); the rook moves too.
pub const QUEEN_CASTLE: u8 = 3;
/// A capture of the piece standing on the to square.
pub const CAPTURE: u8 = 4;
/// An en passant capture: the captured pawn is beside the to square, not on it.
pub const EN_PASSANT: u8 = 5;
/// Pawn promotes to a knight.
pub const KNIGHT_PROMOTION: u8 = 8;
/// Pawn promotes to a bishop.
pub const BISHOP_PROMOTION: u8 = 9;
/// Pawn promotes to a rook.
pub const ROOK_PROMOTION: u8 = 10;
/// Pawn promotes to a queen.
pub const QUEEN_PROMOTION: u8 = 11;
/// Pawn captures and promotes to a knight.
pub const KNIGHT_PROMOTION_CAPTURE: u8 = 12;
/// Pawn captures and promotes to a bishop.
pub const BISHOP_PROMOTION_CAPTURE: u8 = 13;
/// Pawn captures and promotes to a rook.
pub const ROOK_PROMOTION_CAPTURE: u8 = 14;
/// Pawn captures and promotes to a queen.
pub const QUEEN_PROMOTION_CAPTURE: u8 = 15;

/// Flag bit that marks a capture.
const CAPTURE_BIT: u8 = 0b0100;
/// Flag bit that marks a promotion.
const PROMOTION_BIT: u8 = 0b1000;
/// Low 6 bits: one square.
const SQUARE_MASK: u16 = 0x3F;

/// One move: from square, to square and a flag, packed into a `u16`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Move {
    bits: u16,
}

impl Move {
    /// Packs a move. `from` and `to` are 0-63, `flag` is one of the flag constants.
    pub fn new(from: u8, to: u8, flag: u8) -> Move {
        Move {
            bits: from as u16 | (to as u16) << 6 | (flag as u16) << 12,
        }
    }

    /// The square the piece moves from.
    pub fn from(self) -> u8 {
        (self.bits & SQUARE_MASK) as u8
    }

    /// The square the piece moves to.
    pub fn to(self) -> u8 {
        (self.bits >> 6 & SQUARE_MASK) as u8
    }

    /// The flag: what kind of move this is.
    pub fn flag(self) -> u8 {
        (self.bits >> 12) as u8
    }

    /// True for every capture: normal, en passant and promotion captures.
    pub fn is_capture(self) -> bool {
        self.flag() & CAPTURE_BIT != 0
    }

    /// The piece a pawn promotes to, or `None` for a move that is not a promotion.
    pub fn promotion(self) -> Option<PieceKind> {
        if self.flag() & PROMOTION_BIT == 0 {
            return None;
        }
        Some(match self.flag() & 0b11 {
            0 => PieceKind::Knight,
            1 => PieceKind::Bishop,
            2 => PieceKind::Rook,
            _ => PieceKind::Queen,
        })
    }
}

/// Writes the move in UCI form: from, to, and the promotion letter if any (`e2e4`, `e7e8q`).
impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}{}", square_name(self.from()), square_name(self.to()))?;
        if let Some(kind) = self.promotion() {
            // UCI promotion letters are lowercase, the same as black pieces in FEN
            let letter = Piece {
                color: Color::Black,
                kind,
            }
            .to_fen_char();
            write!(f, "{letter}")?;
        }
        Ok(())
    }
}

/// Most moves any chess position can have is 218, so 256 always fits.
pub const MAX_MOVES: usize = 256;

/// The moves of one position, in a fixed array on the stack (no heap allocation).
pub struct MoveList {
    moves: [Move; MAX_MOVES],
    len: usize,
}

impl MoveList {
    /// An empty list.
    pub fn new() -> MoveList {
        MoveList {
            moves: [Move { bits: 0 }; MAX_MOVES],
            len: 0,
        }
    }

    /// Adds a move at the end. Panics if the list is full.
    pub fn push(&mut self, mv: Move) {
        self.moves[self.len] = mv;
        self.len += 1;
    }

    /// How many moves the list holds.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when the list holds no moves.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The moves added so far, in order.
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }
}

/// `MoveList::default()` is the same as `MoveList::new()`.
impl Default for MoveList {
    fn default() -> MoveList {
        MoveList::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every flag in use (6 and 7 are unused).
    const ALL_FLAGS: [u8; 14] = [
        QUIET,
        DOUBLE_PAWN_PUSH,
        KING_CASTLE,
        QUEEN_CASTLE,
        CAPTURE,
        EN_PASSANT,
        KNIGHT_PROMOTION,
        BISHOP_PROMOTION,
        ROOK_PROMOTION,
        QUEEN_PROMOTION,
        KNIGHT_PROMOTION_CAPTURE,
        BISHOP_PROMOTION_CAPTURE,
        ROOK_PROMOTION_CAPTURE,
        QUEEN_PROMOTION_CAPTURE,
    ];

    // ---------- Move ----------

    // 1. the u16 decision: a move takes 2 bytes
    #[test]
    fn move_is_two_bytes() {
        assert_eq!(std::mem::size_of::<Move>(), 2);
    }

    // 2. every from / to / flag combination comes back out unchanged
    #[test]
    fn from_to_flag_round_trip() {
        for from in 0..64 {
            for to in 0..64 {
                for flag in ALL_FLAGS {
                    let mv = Move::new(from, to, flag);
                    assert_eq!(
                        (mv.from(), mv.to(), mv.flag()),
                        (from, to, flag),
                        "from {from} to {to} flag {flag}"
                    );
                }
            }
        }
    }

    // 3. captures: normal, en passant and the 4 promotion captures
    #[test]
    fn is_capture_matches_the_flag() {
        let captures = [
            CAPTURE,
            EN_PASSANT,
            KNIGHT_PROMOTION_CAPTURE,
            BISHOP_PROMOTION_CAPTURE,
            ROOK_PROMOTION_CAPTURE,
            QUEEN_PROMOTION_CAPTURE,
        ];
        for flag in ALL_FLAGS {
            let mv = Move::new(12, 20, flag);
            assert_eq!(mv.is_capture(), captures.contains(&flag), "flag {flag}");
        }
    }

    // 4. promotion piece for each promotion flag, None for the rest
    #[test]
    fn promotion_piece_matches_the_flag() {
        let expected = [
            (KNIGHT_PROMOTION, Some(PieceKind::Knight)),
            (BISHOP_PROMOTION, Some(PieceKind::Bishop)),
            (ROOK_PROMOTION, Some(PieceKind::Rook)),
            (QUEEN_PROMOTION, Some(PieceKind::Queen)),
            (KNIGHT_PROMOTION_CAPTURE, Some(PieceKind::Knight)),
            (BISHOP_PROMOTION_CAPTURE, Some(PieceKind::Bishop)),
            (ROOK_PROMOTION_CAPTURE, Some(PieceKind::Rook)),
            (QUEEN_PROMOTION_CAPTURE, Some(PieceKind::Queen)),
            (QUIET, None),
            (DOUBLE_PAWN_PUSH, None),
            (KING_CASTLE, None),
            (QUEEN_CASTLE, None),
            (CAPTURE, None),
            (EN_PASSANT, None),
        ];
        for (flag, piece) in expected {
            assert_eq!(Move::new(52, 60, flag).promotion(), piece, "flag {flag}");
        }
    }

    // 5. UCI text: squares, plus the promotion letter
    #[test]
    fn display_is_uci_text() {
        assert_eq!(Move::new(12, 28, DOUBLE_PAWN_PUSH).to_string(), "e2e4");
        assert_eq!(Move::new(6, 21, QUIET).to_string(), "g1f3");
        assert_eq!(Move::new(4, 6, KING_CASTLE).to_string(), "e1g1");
        assert_eq!(Move::new(52, 60, QUEEN_PROMOTION).to_string(), "e7e8q");
        assert_eq!(
            Move::new(52, 59, KNIGHT_PROMOTION_CAPTURE).to_string(),
            "e7d8n"
        );
        assert_eq!(Move::new(11, 3, ROOK_PROMOTION).to_string(), "d2d1r");
        assert_eq!(Move::new(36, 43, EN_PASSANT).to_string(), "e5d6");
    }

    // ---------- MoveList ----------

    // 6. a new list is empty
    #[test]
    fn new_list_is_empty() {
        let list = MoveList::new();
        assert_eq!(list.len(), 0);
        assert!(list.is_empty());
        assert!(list.as_slice().is_empty());
    }

    // 7. pushed moves come back in order
    #[test]
    fn push_keeps_order() {
        let a = Move::new(12, 28, DOUBLE_PAWN_PUSH);
        let b = Move::new(6, 21, QUIET);
        let mut list = MoveList::new();
        list.push(a);
        list.push(b);
        assert_eq!(list.len(), 2);
        assert!(!list.is_empty());
        assert_eq!(list.as_slice(), &[a, b]);
    }

    // 8. the list holds exactly MAX_MOVES moves
    #[test]
    fn list_holds_max_moves() {
        let mut list = MoveList::new();
        for _ in 0..MAX_MOVES {
            list.push(Move::new(0, 1, QUIET));
        }
        assert_eq!(list.len(), MAX_MOVES);
    }

    // 9. one more than MAX_MOVES is a bug, so it panics instead of writing past the array
    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn push_past_max_moves_panics() {
        let mut list = MoveList::new();
        for _ in 0..=MAX_MOVES {
            list.push(Move::new(0, 1, QUIET));
        }
    }
}
