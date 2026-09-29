//! Game-over rules: whether the side to move is in check, checkmated or stalemated.
//!
//! A child module of `position`: these are methods on [`Position`] like the ones in
//! `position.rs`, kept in their own file, and they may use that module's private helpers.

use super::Position;
use crate::{movegen::legal_moves, piece::PieceKind, position::opponent};

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
}
