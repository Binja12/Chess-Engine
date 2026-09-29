//! # Perft (CE-8, second subtask CE-77)
//!
//! **What is tested.** `perft::perft` and `perft::divide` through the public API:
//! - depth 0 counts the position itself, depth 1 the legal moves;
//! - the known counts of the start position (to depth 4) and of Kiwipete (to depth 3), small
//!   enough for a debug build; the full reference suite at depth 5 is CE-79;
//! - checkmate and stalemate count 0 below the root;
//! - divide gives one entry per legal first move, its counts add up to perft, and depth 0 gives
//!   an empty list;
//! - both leave the position exactly as they found it.
//!
//! Every expected number was also checked with a separate throwaway perft before this file was
//! written, so a failure points at the implementation, not at the test.
//!
//! Run only these tests with `cargo test -p board --test perft_tests`.

use board::movegen::legal_moves;
use board::moves::Move;
use board::perft::{divide, perft};
use board::position::{Position, START_FEN};

/// A position built to test castling, en passant, promotions, pins and checks.
const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

/// White is checkmated (1. f3 e5 2. g4 Qh4#).
const FOOLS_MATE: &str = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";

/// Black to move has no legal move and is not in check.
const STALEMATE: &str = "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1";

/// Parses a FEN that the test knows is valid.
fn parse(fen: &str) -> Position {
    Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen:?} should parse, got {e:?}"))
}

/// The perft count of `fen` at `depth`.
fn perft_of(fen: &str, depth: u32) -> u64 {
    perft(&mut parse(fen), depth)
}

/// The moves of a divide result, as sorted UCI text.
fn divide_moves(counts: &[(Move, u64)]) -> Vec<String> {
    let mut moves: Vec<String> = counts.iter().map(|(mv, _)| mv.to_string()).collect();
    moves.sort();
    moves
}

/// The legal moves of `pos`, as sorted UCI text.
fn legal_uci(pos: &mut Position) -> Vec<String> {
    let mut moves: Vec<String> = legal_moves(pos)
        .as_slice()
        .iter()
        .map(|mv| mv.to_string())
        .collect();
    moves.sort();
    moves
}

// 1. depth 0 counts the position itself, even when it is checkmate
#[test]
fn depth_zero_counts_the_position_itself() {
    assert_eq!(perft_of(START_FEN, 0), 1);
    assert_eq!(perft_of(FOOLS_MATE, 0), 1);
}

// 2. depth 1 is the number of legal moves
#[test]
fn depth_one_is_the_number_of_legal_moves() {
    for (fen, count) in [(START_FEN, 20), (KIWIPETE, 48)] {
        let mut pos = parse(fen);
        assert_eq!(legal_moves(&mut pos).len(), count, "{fen}");
        assert_eq!(perft(&mut pos, 1), count as u64, "{fen}");
    }
}

// 3. the known counts of the start position
#[test]
fn start_position_matches_the_known_counts() {
    for (depth, count) in [(1, 20), (2, 400), (3, 8_902), (4, 197_281)] {
        assert_eq!(perft_of(START_FEN, depth), count, "depth {depth}");
    }
}

// 4. the known counts of Kiwipete
#[test]
fn kiwipete_matches_the_known_counts() {
    for (depth, count) in [(1, 48), (2, 2_039), (3, 97_862)] {
        assert_eq!(perft_of(KIWIPETE, depth), count, "depth {depth}");
    }
}

// 5. checkmate and stalemate have no legal moves, so nothing below the root
#[test]
fn checkmate_and_stalemate_count_zero_below_the_root() {
    for fen in [FOOLS_MATE, STALEMATE] {
        for depth in 1..=3 {
            assert_eq!(perft_of(fen, depth), 0, "{fen} depth {depth}");
        }
    }
}

// 6. divide gives one entry per legal first move, and its counts add up to perft: from the
//    start, each of White's 20 first moves has 20 replies
#[test]
fn divide_splits_perft_by_first_move() {
    let mut pos = parse(START_FEN);
    let counts = divide(&mut pos, 2);
    assert_eq!(divide_moves(&counts), legal_uci(&mut pos));
    assert!(counts.iter().all(|&(_, count)| count == 20), "{counts:?}");

    let mut pos = parse(KIWIPETE);
    let counts = divide(&mut pos, 3);
    assert_eq!(divide_moves(&counts), legal_uci(&mut pos));
    assert_eq!(counts.iter().map(|&(_, count)| count).sum::<u64>(), 97_862);
}

// 7. at depth 1 every first move counts 1: the position right after it
#[test]
fn divide_at_depth_one_counts_one_per_move() {
    let counts = divide(&mut parse(START_FEN), 1);
    assert_eq!(counts.len(), 20);
    assert!(counts.iter().all(|&(_, count)| count == 1), "{counts:?}");
}

// 8. at depth 0 there is no first move to split by: the list is empty (and computing
//    `depth - 1` on the way must not underflow)
#[test]
fn divide_at_depth_zero_is_empty() {
    assert!(divide(&mut parse(START_FEN), 0).is_empty());
}

// 9. perft and divide play and take back every move; afterwards the position is exactly as
//    before
#[test]
fn perft_and_divide_leave_the_position_unchanged() {
    let mut pos = parse(KIWIPETE);
    let before = pos.clone();
    perft(&mut pos, 3);
    assert_eq!(pos, before);
    divide(&mut pos, 3);
    assert_eq!(pos, before);
}
