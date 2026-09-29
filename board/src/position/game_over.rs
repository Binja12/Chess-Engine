//! Game-over rules: whether the side to move is in check, checkmated or stalemated, and whether
//! the game is drawn by the 50-move rule or because the pieces left can no longer give
//! checkmate.
//!
//! A child module of `position`: these are methods on [`Position`] like the ones in
//! `position.rs`, kept in their own file, and they may use that module's private helpers.

use super::Position;
use crate::{
    bitboard::Bitboard, color::Color, movegen::legal_moves, piece::PieceKind, position::opponent,
};

/// The dark squares: a1, c1, …, h8. A square is dark when its file + rank is even.
const DARK_SQUARES: Bitboard = Bitboard {
    bits: 0xAA55_AA55_AA55_AA55,
};

/// Half-moves without a capture or a pawn move after which the game is drawn: 50 moves each.
const FIFTY_MOVE_LIMIT: u16 = 100;

impl Position {
    /// True if the king of the side to move is attacked. Only that king can be: in a legal
    /// position the side that just moved is never in check.
    pub fn in_check(&self) -> bool {
        self.is_attacked(
            self.pieces(self.side_to_move(), PieceKind::King).lsb(),
            opponent(self.side_to_move()),
        )
    }

    /// True if the side to move is in check and has no legal move: it has lost. Tries moves with
    /// make/unmake (hence `&mut self`); afterwards the position is exactly as before.
    pub fn is_checkmate(&mut self) -> bool {
        self.in_check() && !self.has_legal_move()
    }

    /// True if the side to move is not in check but has no legal move: the game is a draw.
    /// Like [`Position::is_checkmate`], leaves the position exactly as before.
    pub fn is_stalemate(&mut self) -> bool {
        !self.in_check() && !self.has_legal_move()
    }

    /// True if the side to move has at least one legal move. Builds the whole list with
    /// `legal_moves` (every pseudo-legal move is made and unmade) and checks it is not empty.
    fn has_legal_move(&mut self) -> bool {
        !legal_moves(self).is_empty()
    }

    /// True if neither side can checkmate any more, whatever both play: the game is a draw.
    /// That is the case when no pawn, rook or queen is left, and either
    /// - at most one knight or bishop is left (K v K, K+N v K, K+B v K), or
    /// - only bishops are left, all on squares of one color (K+B v K+B with both bishops on
    ///   light squares, or promoted bishops): they only ever attack that color, so the king
    ///   always keeps a free square of the other color.
    ///
    /// Everything else can still end in mate, even when it cannot be forced (K+N+N v K,
    /// K+N v K+N, bishops on opposite colors), so the game goes on. Whose turn it is does not
    /// matter.
    pub fn is_insufficient_material(&self) -> bool {
        let both = |kind| self.pieces(Color::White, kind) | self.pieces(Color::Black, kind);
        // a pawn can still promote, and a rook or a queen can mate
        for kind in [PieceKind::Pawn, PieceKind::Rook, PieceKind::Queen] {
            if !both(kind).is_empty() {
                return false;
            }
        }
        let knights = both(PieceKind::Knight);
        let bishops = both(PieceKind::Bishop);
        // K v K, K+N v K, K+B v K
        if (knights | bishops).count() <= 1 {
            return true;
        }
        // only bishops, all on squares of one color
        knights.is_empty()
            && ((bishops & DARK_SQUARES).is_empty() || (bishops & !DARK_SQUARES).is_empty())
    }

    /// True if 50 moves by each side (100 half-moves) went by without a capture or a pawn move:
    /// the game is a draw. A checkmate on that last move still wins (FIDE's rule, and
    /// Stockfish's), so once the count is reached this also checks for mate, which tries moves
    /// with make/unmake (hence `&mut self`).
    pub fn is_fifty_move_draw(&mut self) -> bool {
        self.halfmove_clock() >= FIFTY_MOVE_LIMIT && !self.is_checkmate()
    }
}

#[cfg(test)]
mod tests {
    use super::DARK_SQUARES;

    // 1. a square is dark exactly when its file + rank is even: a1 and h8 are dark, h1 and a8
    //    are light
    #[test]
    fn dark_squares_have_an_even_file_plus_rank() {
        for sq in 0..64u8 {
            let (file, rank) = (sq % 8, sq / 8);
            assert_eq!(
                DARK_SQUARES.contains(sq),
                (file + rank) % 2 == 0,
                "square {sq}"
            );
        }
    }
}
