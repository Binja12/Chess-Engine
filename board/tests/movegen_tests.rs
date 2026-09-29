//! # Pseudo-legal move generation (CE-7, third subtask)
//!
//! **What is tested.** `generate_moves` through the public API: counts in known positions,
//! each piece's moves, every pawn case (pushes, double pushes, captures, en passant,
//! promotions, both colors), every castling rule, and the `MoveGen` config (presets and
//! builders). "Pseudo-legal" is pinned by a test too: a pinned piece still generates moves;
//! removing moves that leave the king in check is CE-8's legality filter.
//!
//! Moves are compared as UCI text (`e2e4`, `e7e8q`), sorted, so failures read like chess.
//!
//! Run only these tests with `cargo test -p board --test movegen_tests`.

use board::masks::rank_mask;
use board::movegen::{MoveGen, generate_moves};
use board::moves::{CAPTURE, DOUBLE_PAWN_PUSH, EN_PASSANT, KING_CASTLE, Move, QUEEN_CASTLE, QUIET};
use board::piece::PieceKind;
use board::position::{Position, START_FEN};
use board::square::square_from_name;
use std::collections::HashSet;

/// Start position, Kiwipete, perft positions 3-6, en passant and promotion positions.
const VARIED_FENS: [&str; 8] = [
    START_FEN,
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "4k3/8/8/2PpP3/8/8/8/4K3 w - d6 0 1",
    "nr2k3/P7/8/8/8/8/8/4K3 w - - 0 1",
];

/// Parses a FEN that the test knows is valid.
fn parse(fen: &str) -> Position {
    Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen:?} should parse, got {e:?}"))
}

/// Square index from its name, e.g. `sq("e4")`.
fn sq(name: &str) -> u8 {
    square_from_name(name).expect("valid square name")
}

/// The moves `config` generates in `fen`.
fn moves(fen: &str, config: MoveGen) -> Vec<Move> {
    generate_moves(&parse(fen), config).as_slice().to_vec()
}

/// The moves starting on `from`, as sorted UCI text.
fn uci_from(fen: &str, from: &str) -> Vec<String> {
    let from = sq(from);
    let mut list: Vec<String> = moves(fen, MoveGen::ALL)
        .iter()
        .filter(|m| m.from() == from)
        .map(|m| m.to_string())
        .collect();
    list.sort();
    list
}

/// The generated move with this UCI text, if any.
fn find(fen: &str, text: &str) -> Option<Move> {
    moves(fen, MoveGen::ALL)
        .into_iter()
        .find(|m| m.to_string() == text)
}

/// Turns a list of `&str` into sorted `String`s, to compare with `uci_from`.
fn sorted(list: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = list.iter().map(|s| s.to_string()).collect();
    v.sort();
    v
}

/// True if any generated move has this castling flag.
fn has_castle(fen: &str, flag: u8) -> bool {
    moves(fen, MoveGen::ALL).iter().any(|m| m.flag() == flag)
}

// ---------- counts and basics ----------

// 1. the start position has exactly 20 moves
#[test]
fn start_position_has_20_moves() {
    assert_eq!(moves(START_FEN, MoveGen::ALL).len(), 20);
}

// 2. only the side to move generates moves
#[test]
fn only_the_side_to_move_moves() {
    assert!(moves(START_FEN, MoveGen::ALL).iter().all(|m| m.from() < 16));
    let black = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1";
    let list = moves(black, MoveGen::ALL);
    assert_eq!(list.len(), 20);
    assert!(list.iter().all(|m| m.from() >= 48));
}

// 3. no move is generated twice
#[test]
fn no_duplicate_moves() {
    for fen in VARIED_FENS {
        let list = moves(fen, MoveGen::ALL);
        let distinct: HashSet<Move> = list.iter().copied().collect();
        assert_eq!(distinct.len(), list.len(), "{fen}");
    }
}

// ---------- pieces ----------

// 4. knight: all 8 squares in the centre, 2 in a corner
#[test]
fn knight_moves() {
    let centre = "4k3/8/8/8/3N4/8/8/4K3 w - - 0 1";
    assert_eq!(
        uci_from(centre, "d4"),
        sorted(&[
            "d4b3", "d4b5", "d4c2", "d4c6", "d4e2", "d4e6", "d4f3", "d4f5"
        ])
    );
    let corner = "4k3/8/8/8/8/8/8/N3K3 w - - 0 1";
    assert_eq!(uci_from(corner, "a1"), sorted(&["a1b3", "a1c2"]));
}

// 5. a piece never lands on its own piece; landing on an enemy piece is a capture
#[test]
fn own_pieces_block_and_enemy_pieces_are_captured() {
    let fen = "4k3/8/8/8/3N4/1P3p2/8/4K3 w - - 0 1";
    assert_eq!(
        uci_from(fen, "d4"),
        sorted(&["d4b5", "d4c2", "d4c6", "d4e2", "d4e6", "d4f3", "d4f5"])
    );
    assert_eq!(find(fen, "d4f3").unwrap().flag(), CAPTURE);
    assert_eq!(find(fen, "d4e6").unwrap().flag(), QUIET);
}

// 6. rook: stops at blockers, captures the first enemy piece, nothing beyond it
#[test]
fn rook_moves_stop_at_blockers() {
    let fen = "4k3/8/8/3p4/8/8/3R4/K7 w - - 0 1";
    assert_eq!(
        uci_from(fen, "d2"),
        sorted(&[
            "d2d3", "d2d4", "d2d5", "d2d1", "d2c2", "d2b2", "d2a2", "d2e2", "d2f2", "d2g2", "d2h2"
        ])
    );
    assert_eq!(find(fen, "d2d5").unwrap().flag(), CAPTURE);
}

// 7. queen on an open board: rook moves + bishop moves
#[test]
fn queen_on_open_board_has_27_moves() {
    assert_eq!(uci_from("K7/8/8/8/3Q4/8/8/7k w - - 0 1", "d4").len(), 27);
}

// 8. king: the 8 squares around it, minus its own pieces
#[test]
fn king_moves() {
    let fen = "4k3/8/8/8/3K4/3P4/8/8 w - - 0 1";
    assert_eq!(
        uci_from(fen, "d4"),
        sorted(&["d4c3", "d4e3", "d4c4", "d4e4", "d4c5", "d4d5", "d4e5"])
    );
}

// ---------- pawns ----------

// 9. single push; double push only from the start rank, with its flag
#[test]
fn pawn_single_and_double_push() {
    assert_eq!(find(START_FEN, "e2e3").unwrap().flag(), QUIET);
    assert_eq!(find(START_FEN, "e2e4").unwrap().flag(), DOUBLE_PAWN_PUSH);
    assert_eq!(
        uci_from("4k3/8/8/8/8/4P3/8/4K3 w - - 0 1", "e3"),
        sorted(&["e3e4"])
    );
}

// 10. a blocked pawn: nothing if the square ahead is taken, no double push if the 4th rank is
#[test]
fn blocked_pawns() {
    assert!(uci_from("4k3/8/8/8/8/4p3/4P3/4K3 w - - 0 1", "e2").is_empty());
    assert_eq!(
        uci_from("4k3/8/8/8/4p3/8/4P3/4K3 w - - 0 1", "e2"),
        sorted(&["e2e3"])
    );
}

// 11. captures only diagonally forward onto enemy pieces, no wrap between the a- and h-files
#[test]
fn pawn_captures() {
    let fen = "4k3/8/8/8/8/3p1P2/4P3/4K3 w - - 0 1";
    assert_eq!(uci_from(fen, "e2"), sorted(&["e2d3", "e2e3", "e2e4"]));
    assert_eq!(find(fen, "e2d3").unwrap().flag(), CAPTURE);

    // a2 must not "capture" h2, and h2 must not "capture" b3 (bit shifts that wrap around)
    assert_eq!(
        uci_from("4k3/8/8/8/8/8/P6p/4K3 w - - 0 1", "a2"),
        sorted(&["a2a3", "a2a4"])
    );
    assert_eq!(
        uci_from("4k3/8/8/8/8/1p6/7P/4K3 w - - 0 1", "h2"),
        sorted(&["h2h3", "h2h4"])
    );
}

// 12. en passant only when the en passant square is set; two pawns can both take
#[test]
fn en_passant() {
    let fen = "4k3/8/8/2PpP3/8/8/8/4K3 w - d6 0 1";
    assert_eq!(find(fen, "c5d6").unwrap().flag(), EN_PASSANT);
    assert_eq!(find(fen, "e5d6").unwrap().flag(), EN_PASSANT);

    let no_ep = "4k3/8/8/2PpP3/8/8/8/4K3 w - - 0 1";
    assert!(find(no_ep, "c5d6").is_none());
    assert!(find(no_ep, "e5d6").is_none());
}

// 13. a promotion push is 4 moves (q, r, b, n) and never a plain push
#[test]
fn promotion_gives_four_moves() {
    let fen = "4k3/P7/8/8/8/8/8/4K3 w - - 0 1";
    assert_eq!(
        uci_from(fen, "a7"),
        sorted(&["a7a8b", "a7a8n", "a7a8q", "a7a8r"])
    );
    let list = moves(fen, MoveGen::ALL);
    assert!(
        list.iter()
            .filter(|m| m.from() == sq("a7"))
            .all(|m| m.promotion().is_some())
    );
}

// 14. promotion captures: 4 per captured square, flagged as capture and promotion
#[test]
fn promotion_captures() {
    // a8 free: 4 pushes + 4 captures on b8
    assert_eq!(uci_from("1r2k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7").len(), 8);
    // a8 taken: only the 4 captures on b8
    let fen = "nr2k3/P7/8/8/8/8/8/4K3 w - - 0 1";
    assert_eq!(
        uci_from(fen, "a7"),
        sorted(&["a7b8b", "a7b8n", "a7b8q", "a7b8r"])
    );
    for m in moves(fen, MoveGen::ALL)
        .iter()
        .filter(|m| m.from() == sq("a7"))
    {
        assert!(m.is_capture() && m.promotion().is_some(), "{m}");
    }
}

// 15. black pawns move down, double push from rank 7, promote on rank 1
#[test]
fn black_pawns() {
    let fen = "4k3/4p3/8/8/8/8/p7/4K3 b - - 0 1";
    assert_eq!(uci_from(fen, "e7"), sorted(&["e7e5", "e7e6"]));
    assert_eq!(find(fen, "e7e5").unwrap().flag(), DOUBLE_PAWN_PUSH);
    assert_eq!(
        uci_from(fen, "a2"),
        sorted(&["a2a1b", "a2a1n", "a2a1q", "a2a1r"])
    );
}

// ---------- castling ----------

// 16. both sides can castle both ways; the move is the king's, with its flag
#[test]
fn castling_both_ways() {
    let white = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
    assert_eq!(find(white, "e1g1").unwrap().flag(), KING_CASTLE);
    assert_eq!(find(white, "e1c1").unwrap().flag(), QUEEN_CASTLE);
    let black = "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1";
    assert_eq!(find(black, "e8g8").unwrap().flag(), KING_CASTLE);
    assert_eq!(find(black, "e8c8").unwrap().flag(), QUEEN_CASTLE);
}

// 17. no right, no castling
#[test]
fn castling_needs_the_right() {
    let none = "r3k2r/8/8/8/8/8/8/R3K2R w kq - 0 1";
    assert!(!has_castle(none, KING_CASTLE) && !has_castle(none, QUEEN_CASTLE));
    let only_king_side = "r3k2r/8/8/8/8/8/8/R3K2R w Kkq - 0 1";
    assert!(has_castle(only_king_side, KING_CASTLE));
    assert!(!has_castle(only_king_side, QUEEN_CASTLE));
}

// 18. every square between king and rook must be empty, b1 included for the long side
#[test]
fn castling_needs_empty_squares() {
    let both_blocked = "r3k2r/8/8/8/8/8/8/RN2K1NR w KQ - 0 1";
    assert!(!has_castle(both_blocked, KING_CASTLE) && !has_castle(both_blocked, QUEEN_CASTLE));
    let b1_taken = "r3k2r/8/8/8/8/8/8/RN2K2R w KQ - 0 1";
    assert!(has_castle(b1_taken, KING_CASTLE));
    assert!(!has_castle(b1_taken, QUEEN_CASTLE));
}

// 19. no castling out of check
#[test]
fn no_castling_in_check() {
    let fen = "r3k2r/8/8/8/4r3/8/8/R3K2R w KQ - 0 1";
    assert!(!has_castle(fen, KING_CASTLE) && !has_castle(fen, QUEEN_CASTLE));
}

// 20. no castling through an attacked square (f1 for short, d1 for long)
#[test]
fn no_castling_through_attacked_square() {
    let f1_attacked = "4kr2/8/8/8/8/8/8/R3K2R w KQ - 0 1";
    assert!(!has_castle(f1_attacked, KING_CASTLE));
    assert!(has_castle(f1_attacked, QUEEN_CASTLE));
    let d1_attacked = "3rk3/8/8/8/8/8/8/R3K2R w KQ - 0 1";
    assert!(has_castle(d1_attacked, KING_CASTLE));
    assert!(!has_castle(d1_attacked, QUEEN_CASTLE));
}

// 21. b1 attacked but empty: long castling is still fine (the king never crosses b1)
#[test]
fn attacked_b1_does_not_stop_long_castling() {
    assert!(has_castle(
        "1r2k3/8/8/8/8/8/8/R3K2R w KQ - 0 1",
        QUEEN_CASTLE
    ));
}

// 22. no castling onto an attacked square (g1 / c1)
#[test]
fn no_castling_into_attack() {
    assert!(!has_castle(
        "4k1r1/8/8/8/8/8/8/R3K2R w KQ - 0 1",
        KING_CASTLE
    ));
    assert!(!has_castle(
        "2r1k3/8/8/8/8/8/8/R3K2R w KQ - 0 1",
        QUEEN_CASTLE
    ));
}

// ---------- pseudo-legal contract ----------

// 23. a pinned piece still moves: filtering moves that expose the king is CE-8, not here
#[test]
fn pinned_pieces_still_generate_moves() {
    let fen = "4r1k1/8/8/8/8/8/4N3/4K3 w - - 0 1";
    assert_eq!(uci_from(fen, "e2").len(), 6);
}

// ---------- MoveGen config ----------

// 24. CAPTURES and QUIETS split ALL exactly: together all of it, no move in both
#[test]
fn captures_and_quiets_split_all_moves() {
    for fen in VARIED_FENS {
        let all: HashSet<Move> = moves(fen, MoveGen::ALL).into_iter().collect();
        let noisy: HashSet<Move> = moves(fen, MoveGen::CAPTURES).into_iter().collect();
        let quiet: HashSet<Move> = moves(fen, MoveGen::QUIETS).into_iter().collect();
        assert!(noisy.is_disjoint(&quiet), "{fen}: a move is in both");
        let union: HashSet<Move> = noisy.union(&quiet).copied().collect();
        assert_eq!(union, all, "{fen}");
    }
}

// 25. CAPTURES holds only captures and promotions; QUIETS holds neither
#[test]
fn captures_are_noisy_and_quiets_are_not() {
    for fen in VARIED_FENS {
        for m in moves(fen, MoveGen::CAPTURES) {
            assert!(m.is_capture() || m.promotion().is_some(), "{fen}: {m}");
        }
        for m in moves(fen, MoveGen::QUIETS) {
            assert!(!m.is_capture() && m.promotion().is_none(), "{fen}: {m}");
        }
    }
}

// 26. `only` keeps the given piece kinds; castling counts as a king move
#[test]
fn only_restricts_to_the_given_piece_kinds() {
    let start = parse(START_FEN);
    let pawns = generate_moves(&start, MoveGen::ALL.only(&[PieceKind::Pawn]));
    assert_eq!(pawns.len(), 16);
    assert!(
        pawns
            .as_slice()
            .iter()
            .all(|m| start.piece_at(m.from()).unwrap().kind == PieceKind::Pawn)
    );
    assert_eq!(
        generate_moves(&start, MoveGen::ALL.only(&[PieceKind::Knight])).len(),
        4
    );

    let castling = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
    let king = moves(castling, MoveGen::ALL.only(&[PieceKind::King]));
    assert!(king.iter().any(|m| m.flag() == KING_CASTLE));
    assert!(king.iter().any(|m| m.flag() == QUEEN_CASTLE));
    let rooks = moves(castling, MoveGen::ALL.only(&[PieceKind::Rook]));
    assert!(
        rooks
            .iter()
            .all(|m| m.flag() == QUIET || m.flag() == CAPTURE)
    );
}

// 27. `landing_on` keeps only moves ending in the given squares
#[test]
fn landing_on_restricts_destinations() {
    // start position, rank 4 only: the 8 double pushes
    let list = moves(START_FEN, MoveGen::ALL.landing_on(rank_mask(3)));
    assert_eq!(list.len(), 8);
    assert!(list.iter().all(|m| m.flag() == DOUBLE_PAWN_PUSH));
}

// 28. `only` with several kinds gives all of them in one list; chained calls keep narrowing
#[test]
fn only_accepts_several_kinds_and_narrows_when_chained() {
    let pawns_and_knights = MoveGen::ALL.only(&[PieceKind::Pawn, PieceKind::Knight]);
    assert_eq!(moves(START_FEN, pawns_and_knights).len(), 16 + 4);

    let narrowed = pawns_and_knights.only(&[PieceKind::Knight, PieceKind::Queen]);
    assert_eq!(
        moves(START_FEN, narrowed).len(),
        4,
        "only knights are in both"
    );
}
