//! Perft ("performance test"): counts every sequence of legal moves `depth` half-moves long.
//!
//! The counts for a set of standard test positions are known exactly, so matching them shows
//! that move generation and make/unmake follow the rules; one missing, extra or wrongly played
//! move anywhere in the tree changes the number. [`divide`] splits a count by first move, to
//! find where a wrong count comes from. `examples/perft.rs` runs it from the command line in
//! the same format as Stockfish's `go perft`.

use crate::movegen::legal_moves;
use crate::moves::Move;
use crate::position::Position;

/// How many sequences of legal moves `depth` half-moves long start from `pos`: the positions at
/// the bottom of the move tree, counted once per path (two move orders that reach the same
/// position count twice). Depth 0 is 1, the position itself. Plays and takes back every move,
/// so `pos` is unchanged afterwards.
pub fn perft(pos: &mut Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut branches = 0;
    for mv in legal_moves(pos).as_slice() {
        let undo = pos.make_move(*mv);
        branches += perft(pos, depth - 1);
        pos.unmake_move(*mv, undo);
    }
    branches
}

/// Like [`perft`], but instead of one total it gives a count for each first move.
///
/// Example: the start position at depth 2 gives 20 pairs, one per White first move, each with
/// 20 (Black's replies to it). The counts add up to `perft` at the same depth (400). When a
/// total is wrong, the pair with the wrong count shows which first move the bug is under.
///
/// Depth 0 has no first move, so it returns an empty list. Leaves `pos` unchanged.
pub fn divide(pos: &mut Position, depth: u32) -> Vec<(Move, u64)> {
    let mut counts = Vec::new();
    if depth == 0 {
        return counts;
    }
    for mv in legal_moves(pos).as_slice() {
        let undo = pos.make_move(*mv);
        let count = perft(pos, depth - 1);
        pos.unmake_move(*mv, undo);
        counts.push((*mv, count));
    }
    counts
}
