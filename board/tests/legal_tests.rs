//! # Legal move filter (CE-8, first subtask CE-75)
//!
//! **What is tested.** `movegen::legal_moves` and `Position::is_legal` through the public API:
//! - the number of legal moves in each reference position (perft depth 1, known values);
//! - pins: a pinned piece keeps only the moves along the pin line (a pinned pawn may push along
//!   it but not capture en passant off it);
//! - the king never steps onto an attacked square, not even by backing away along the line of
//!   the piece that checks it;
//! - in check only moves that end the check are legal, and in double check only king moves;
//! - castling stays legal, checkmate and stalemate have no legal moves, and `legal_moves` leaves
//!   the position exactly as it found it.
//!
//! Every expected move list was also checked with a separate throwaway filter before this file
//! was written, so a failure points at the implementation, not at the test.
//!
//! Run only these tests with `cargo test -p board --test legal_tests`.

use board::movegen::{MoveGen, generate_moves, legal_moves};
use board::moves::Move;
use board::position::{Position, START_FEN};

/// Parses a FEN that the test knows is valid.
fn parse(fen: &str) -> Position {
    Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen:?} should parse, got {e:?}"))
}

/// The legal moves of `fen`, as sorted UCI text.
fn legal_uci(fen: &str) -> Vec<String> {
    let mut pos = parse(fen);
    let mut list: Vec<String> = legal_moves(&mut pos)
        .as_slice()
        .iter()
        .map(|m| m.to_string())
        .collect();
    list.sort();
    list
}

/// `moves` as sorted owned strings, to compare with `legal_uci`.
fn sorted(moves: &[&str]) -> Vec<String> {
    let mut list: Vec<String> = moves.iter().map(|m| m.to_string()).collect();
    list.sort();
    list
}

/// The pseudo-legal move with this UCI text, taken from `generate_moves` so its flag is the
/// real one. Panics if the position has no such move.
fn pseudo_move(pos: &Position, text: &str) -> Move {
    generate_moves(pos, MoveGen::ALL)
        .as_slice()
        .iter()
        .copied()
        .find(|m| m.to_string() == text)
        .unwrap_or_else(|| panic!("{text} is not a pseudo-legal move in {}", pos.to_fen()))
}

// 1. perft depth 1: the known number of legal moves in each reference position
#[test]
fn reference_positions_have_the_known_number_of_legal_moves() {
    let cases = [
        (START_FEN, 20),
        (
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            48,
        ),
        ("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 14),
        (
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            6,
        ),
        (
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            44,
        ),
        (
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            46,
        ),
    ];
    for (fen, count) in cases {
        let mut pos = parse(fen);
        assert_eq!(legal_moves(&mut pos).len(), count, "{fen}");
    }
}

// 2. a pinned piece cannot leave the line between the pinner and its king: the e2 knight has 6
//    pseudo-legal moves and none of them is legal
#[test]
fn pinned_knight_cannot_move() {
    assert_eq!(
        legal_uci("4r1k1/8/8/8/8/8/4N3/4K3 w - - 0 1"),
        sorted(&["e1d1", "e1d2", "e1f1", "e1f2"])
    );
}

// 3. ... but a pinned piece may move along that line, up to capturing the pinner
#[test]
fn pinned_rook_moves_only_along_the_pin_line() {
    assert_eq!(
        legal_uci("4r1k1/8/8/8/8/8/4R3/4K3 w - - 0 1"),
        sorted(&[
            "e2e3", "e2e4", "e2e5", "e2e6", "e2e7", "e2e8", "e1d1", "e1d2", "e1f1", "e1f2",
        ])
    );
}

// 4. the king never moves onto an attacked square (d1 and d2 are on the d8 rook's file)
#[test]
fn king_cannot_step_into_check() {
    assert_eq!(
        legal_uci("3rk3/8/8/8/8/8/8/4K3 w - - 0 1"),
        sorted(&["e1e2", "e1f1", "e1f2"])
    );
}

// 5. a king in check cannot back away along the checking line: e1 looks safe while the king
//    still stands on e2, but once it moves the e8 rook sees through to e1
#[test]
fn king_cannot_back_away_along_the_checking_line() {
    assert_eq!(
        legal_uci("4r1k1/8/8/8/8/8/4K3/8 w - - 0 1"),
        sorted(&["e2d1", "e2d2", "e2d3", "e2f1", "e2f2", "e2f3"])
    );
}

// 6. in check only moves that end the check are legal: capture the checker (Rxe4), block (Be2),
//    or move the king to a safe square (e2 is still on the rook's file)
#[test]
fn in_check_only_moves_that_end_the_check_are_legal() {
    assert_eq!(
        legal_uci("4k3/8/8/8/R3r3/8/8/3BK3 w - - 0 1"),
        sorted(&["a4e4", "d1e2", "e1d2", "e1f1", "e1f2"])
    );
}

// 7. in double check (the e8 rook and the d3 knight) only the king can move: capturing the knight
//    (Qxd3) or blocking the rook (Qe2) still leaves the other check, and f2 is covered by the
//    knight
#[test]
fn in_double_check_only_the_king_moves() {
    assert_eq!(
        legal_uci("4r1k1/8/8/8/8/3n4/8/3QK3 w - - 0 1"),
        sorted(&["e1d2", "e1f1"])
    );
}

// 8. the d4 pawn is pinned on the d-file: pushing to d3 stays on the pin line and is legal, taking
//    en passant on e3 leaves the line and is not; the free f4 pawn may take
#[test]
fn pinned_pawn_may_push_along_the_pin_but_not_capture_off_it() {
    let moves = legal_uci("3k4/8/8/8/3pPp2/8/8/3R3K b - e3 0 1");
    assert!(moves.contains(&"d4d3".to_string()), "{moves:?}");
    assert!(moves.contains(&"f4e3".to_string()), "{moves:?}");
    assert!(!moves.contains(&"d4e3".to_string()), "{moves:?}");
    assert_eq!(moves.len(), 8, "{moves:?}");
}

// 9. castling that generate_moves allows stays legal (it already checked the king's path)
#[test]
fn castling_stays_legal() {
    let moves = legal_uci("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    assert!(moves.contains(&"e1g1".to_string()), "{moves:?}");
    assert!(moves.contains(&"e1c1".to_string()), "{moves:?}");
}

// 10. checkmate (fool's mate) and stalemate: no legal moves at all
#[test]
fn checkmate_and_stalemate_have_no_legal_moves() {
    let mate = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";
    assert_eq!(legal_uci(mate), Vec::<String>::new());
    let stalemate = "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1";
    assert_eq!(legal_uci(stalemate), Vec::<String>::new());
}

// 11. legal_moves plays and takes back every move; afterwards the position is exactly as before
#[test]
fn legal_moves_leaves_the_position_unchanged() {
    let mut pos = parse("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1");
    let before = pos.clone();
    legal_moves(&mut pos);
    assert_eq!(pos, before);
}

// 12. is_legal answers for a single move: the pinned knight's move is pseudo-legal but not
//     legal, the king's move is both, and asking does not change the position
#[test]
fn is_legal_checks_a_single_move() {
    let mut pos = parse("4r1k1/8/8/8/8/8/4N3/4K3 w - - 0 1");
    let before = pos.clone();
    let knight = pseudo_move(&pos, "e2c3");
    let king = pseudo_move(&pos, "e1d1");
    assert!(!pos.is_legal(knight));
    assert!(pos.is_legal(king));
    assert_eq!(pos, before);
}
