//! # Game-over rules (CE-257: subtasks CE-258 check, checkmate, stalemate; CE-259 50-move rule;
//! # CE-260 threefold repetition; CE-261 insufficient material)
//!
//! **What is tested.** `Position::in_check`, `is_checkmate`, `is_stalemate`,
//! `is_insufficient_material`, `is_fifty_move_draw` and `is_threefold_repetition` through the
//! public API:
//! - every piece kind can give check, to either side, and two checkers at once are check too;
//! - no check when nothing attacks the king: pawns attack only diagonally forward, and a piece
//!   of either color blocks a line;
//! - a move can give check by uncovering a line (discovered check);
//! - checkmate: the king has no safe square and the check can be neither captured nor blocked,
//!   including a double check where one of the checkers could be captured;
//! - stalemate: not in check and no legal move, including a side whose other piece is pinned;
//! - a check with a way out (the king steps away, the checker is captured, a piece blocks) is
//!   neither;
//! - a mate reached by playing moves, not only one parsed from a FEN;
//! - ordinary positions are not over, and the queries leave the position exactly as it was;
//! - insufficient material: K v K, K+N v K, K+B v K, and bishops only, all on one square color
//!   (promoted ones too), with either side to move; everything else can still mate, even when
//!   it cannot be forced (K+N+N v K, K+N v K+N, bishops on opposite colors); and capturing the
//!   last pawn turns a position into a draw;
//! - the 50-move rule: 100 half-moves without a capture or a pawn move is a draw (99 is not), a
//!   pawn move or a capture resets the count, and a checkmate on the last move still wins (a
//!   check does not stop the draw);
//! - threefold repetition: the start position is the first occurrence; castling rights and a
//!   legal en passant capture make positions different, an en passant square without a legal
//!   capture does not; `unmake_move` takes a position back out of the history; nothing before
//!   the FEN counts.
//!
//! Every check, mate and stalemate position was also checked with the `chess` crate (game
//! status, number of checking pieces, legal moves) before this file was written, so a failure
//! points at the implementation, not at the test. So were the repetition sequences (its
//! `Game::can_declare_draw`), except the one with an illegal en passant capture, where the crate
//! differs from FIDE (see test 18). The `chess` crate has no insufficient material rule; those
//! cases follow FIDE's dead-position rule (no sequence of legal moves can mate).
//!
//! Run only these tests with `cargo test -p board --test game_over_tests`.

use board::movegen::{MoveGen, generate_moves};
use board::moves::Move;
use board::position::{Position, START_FEN};

/// Kiwipete (perft position 2): a busy middlegame position.
const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

/// Fool's mate: Black's queen mates White after 1. f3 e5 2. g4 Qh4#.
const FOOLS_MATE: &str = "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3";

/// Stalemate although Black still has a knight: the e4 bishop pins it to the king, and the
/// king's other squares (a7, b8) are covered by the b6 pawn and the c7 king.
const PINNED_KNIGHT_STALEMATE: &str = "k7/1nK5/1P6/8/4B3/8/8/8 b - - 0 1";

/// The knights go out and come back: after these 4 moves the position is the same as before.
const KNIGHT_DANCE: [&str; 4] = ["g1f3", "g8f6", "f3g1", "f6g8"];

/// Parses a FEN that the test knows is valid.
fn parse(fen: &str) -> Position {
    Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen:?} should parse, got {e:?}"))
}

/// The move with this UCI text (e.g. `e2e4`), taken from `generate_moves` so its flag is the
/// real one. Panics if the position has no such legal move.
fn find_move(pos: &mut Position, text: &str) -> Move {
    let mv = generate_moves(pos, MoveGen::ALL)
        .as_slice()
        .iter()
        .copied()
        .find(|m| m.to_string() == text)
        .unwrap_or_else(|| panic!("{text} is not a pseudo-legal move in {}", pos.to_fen()));
    assert!(pos.is_legal(mv), "{text} is not legal in {}", pos.to_fen());
    mv
}

/// Plays the move with this UCI text (see `find_move`).
fn play(pos: &mut Position, text: &str) {
    let mv = find_move(pos, text);
    pos.make_move(mv);
}

/// `fen` as written (White to move) and the same position with Black to move.
fn both_sides_to_move(fen: &str) -> [String; 2] {
    [fen.to_string(), fen.replace(" w ", " b ")]
}

// 1. every piece kind can give check, to either side; two checkers at once are check too
#[test]
fn every_piece_kind_can_give_check() {
    let cases = [
        ("black pawn", "7k/8/8/3p4/4K3/8/8/8 w - - 0 1"),
        ("black knight", "7k/8/8/8/8/5n2/8/4K3 w - - 0 1"),
        ("black bishop", "7k/8/8/8/1b6/8/8/4K3 w - - 0 1"),
        ("black rook", "4r2k/8/8/8/8/8/8/4K3 w - - 0 1"),
        ("black queen", "7k/8/8/8/8/8/8/q3K3 w - - 0 1"),
        ("white rook", "4k3/8/8/8/8/8/8/4RK2 b - - 0 1"),
        ("white pawn", "8/8/8/3k4/4P3/8/8/4K3 b - - 0 1"),
        (
            "double check: the e8 rook and the d3 knight",
            "4r1k1/8/8/8/8/3n4/8/3QK3 w - - 0 1",
        ),
    ];
    for (checker, fen) in cases {
        assert!(parse(fen).in_check(), "{checker}: {fen}");
    }
}

// 2. no check when nothing attacks the king: white pawns attack only diagonally upwards (not the
//    square in front of them, not downwards), and a piece of either color blocks a line
#[test]
fn no_check_when_nothing_attacks_the_king() {
    let cases = [
        ("start position", START_FEN),
        (
            "the king stands in front of the d4 pawn and diagonally below the e6 pawn",
            "8/8/4P3/3k4/3P4/8/8/4K3 b - - 0 1",
        ),
        (
            "the e8 rook's line is blocked by White's own pawn",
            "4r2k/8/8/8/4P3/8/8/4K3 w - - 0 1",
        ),
        (
            "the e8 rook's line is blocked by a black pawn",
            "4r2k/8/8/8/4p3/8/8/4K3 w - - 0 1",
        ),
    ];
    for (why, fen) in cases {
        assert!(!parse(fen).in_check(), "{why}: {fen}");
    }
}

// 3. a move can give check without the moving piece attacking the king: the knight leaves the
//    e-file and uncovers the e1 rook (discovered check)
#[test]
fn a_move_can_uncover_a_check() {
    let mut pos = parse("4k3/8/8/8/8/8/4N3/4R1K1 w - - 0 1");
    assert!(!pos.in_check());
    play(&mut pos, "e2c3");
    assert!(pos.in_check(), "{}", pos.to_fen());
}

// 4. checkmate: in check, no safe square for the king, and the check can be neither captured
//    nor blocked
#[test]
fn checkmates() {
    let cases = [
        ("fool's mate", FOOLS_MATE),
        (
            "back-rank mate: the king is boxed in by its own pawns",
            "R5k1/5ppp/8/8/8/8/8/6K1 b - - 1 1",
        ),
        (
            "smothered mate: the knight checks, the king's own pieces fill its squares",
            "6rk/5Npp/8/8/8/8/8/6K1 b - - 0 1",
        ),
        (
            "the checking queen is defended, so the king cannot take it",
            "k7/1Q6/1K6/8/8/8/8/8 b - - 0 1",
        ),
        (
            "double check: Bxd6 takes the knight, but the e1 rook still checks",
            "3qkb2/3p1p2/3N4/8/8/8/8/4R1K1 b - - 0 1",
        ),
    ];
    for (why, fen) in cases {
        let mut pos = parse(fen);
        assert!(pos.in_check(), "{why}: {fen}");
        assert!(pos.is_checkmate(), "{why}: {fen}");
        assert!(!pos.is_stalemate(), "{why}: {fen}");
    }
}

// 5. stalemate: not in check, but no legal move; also when the side still has a piece that
//    cannot move because it is pinned
#[test]
fn stalemates() {
    let cases = [
        (
            "the queen and the g6 king cover g8, g7 and h7",
            "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1",
        ),
        (
            "the a7 pawn covers b8, the a6 king covers b7 and guards the pawn",
            "k7/P7/K7/8/8/8/8/8 b - - 0 1",
        ),
        ("Black's knight is pinned", PINNED_KNIGHT_STALEMATE),
    ];
    for (why, fen) in cases {
        let mut pos = parse(fen);
        assert!(!pos.in_check(), "{why}: {fen}");
        assert!(pos.is_stalemate(), "{why}: {fen}");
        assert!(!pos.is_checkmate(), "{why}: {fen}");
    }
}

// 6. a check with a way out is neither mate nor stalemate. Each position has exactly one kind of
//    way out: the king steps away, the checker is captured (by the king or by another piece), or
//    a piece blocks. Without the blocking bishop the last one is mate.
#[test]
fn check_with_a_way_out_is_not_mate() {
    let cases = [
        (
            "the king steps off the e-file",
            "4k3/8/8/8/8/8/8/4RK2 b - - 0 1",
        ),
        (
            "the king takes the undefended queen",
            "4k3/8/8/8/8/8/6q1/7K w - - 0 1",
        ),
        (
            "the a5 rook takes the checking rook",
            "6k1/8/8/R7/8/8/5PPP/r5K1 w - - 0 1",
        ),
        (
            "the bishop blocks on d1 or f1",
            "6k1/8/8/8/8/8/4BPPP/r5K1 w - - 0 1",
        ),
    ];
    for (why, fen) in cases {
        let mut pos = parse(fen);
        assert!(pos.in_check(), "{why}: {fen}");
        assert!(!pos.is_checkmate(), "{why}: {fen}");
        assert!(!pos.is_stalemate(), "{why}: {fen}");
    }
    // the same back rank without the bishop: nothing can block, so it is mate
    assert!(parse("6k1/8/8/8/8/8/5PPP/r5K1 w - - 0 1").is_checkmate());
}

// 7. a mate reached by playing moves, not only one parsed from a FEN (scholar's mate): the game
//    is not over after any of the first six moves, and Qxf7 mates
#[test]
fn scholars_mate_played_move_by_move() {
    let mut pos = parse(START_FEN);
    for text in ["e2e4", "e7e5", "f1c4", "b8c6", "d1h5", "g8f6"] {
        play(&mut pos, text);
        assert!(!pos.is_checkmate(), "after {text}: {}", pos.to_fen());
        assert!(!pos.is_stalemate(), "after {text}: {}", pos.to_fen());
    }
    play(&mut pos, "h5f7");
    assert!(pos.is_checkmate(), "{}", pos.to_fen());
}

// 8. ordinary positions are not over
#[test]
fn ordinary_positions_are_not_over() {
    for fen in [START_FEN, KIWIPETE] {
        let mut pos = parse(fen);
        assert!(!pos.in_check(), "{fen}");
        assert!(!pos.is_checkmate(), "{fen}");
        assert!(!pos.is_stalemate(), "{fen}");
    }
}

// 9. is_checkmate and is_stalemate try moves with make/unmake; afterwards the position is
//    exactly as before, also in a mate and a stalemate, where every move is tried and none is
//    legal
#[test]
fn the_queries_leave_the_position_unchanged() {
    for fen in [START_FEN, KIWIPETE, FOOLS_MATE, PINNED_KNIGHT_STALEMATE] {
        let mut pos = parse(fen);
        let before = pos.clone();
        pos.is_checkmate();
        pos.is_stalemate();
        assert_eq!(pos, before, "{fen}");
    }
}

// ---------- insufficient material (CE-261) ----------

// 10. no pawn, rook or queen, and either at most one knight or bishop, or only bishops all on
//     one square color: nobody can mate any more. Whose turn it is does not matter.
#[test]
fn insufficient_material() {
    let cases = [
        ("K v K", "8/8/8/4k3/8/8/8/4K3 w - - 0 1"),
        ("K+N v K", "8/8/8/4k3/8/8/8/4KN2 w - - 0 1"),
        ("K v K+N", "8/8/8/4k3/8/2n5/8/4K3 w - - 0 1"),
        ("K+B v K", "8/8/8/4k3/8/8/8/2B1K3 w - - 0 1"),
        (
            "K+B v K+B, both bishops on dark squares (c1, f8)",
            "5b2/8/8/4k3/8/8/8/2B1K3 w - - 0 1",
        ),
        (
            "K+B v K+B, both bishops on light squares (f1, c8)",
            "2b5/8/8/4k3/8/8/8/4KB2 w - - 0 1",
        ),
        (
            "three bishops on dark squares (c1, e3, g5), so two were promoted",
            "8/8/8/4k1B1/8/4B3/8/2B1K3 w - - 0 1",
        ),
    ];
    for (why, fen) in cases {
        for fen in both_sides_to_move(fen) {
            assert!(parse(&fen).is_insufficient_material(), "{why}: {fen}");
        }
    }
}

// 11. everything else can still end in mate, even when it cannot be forced: a pawn can promote,
//     and the losing side's own piece can block its king's last free square (K+N+N v K,
//     K+N v K+N, K+B v K+N, bishops on opposite colors)
#[test]
fn sufficient_material() {
    let cases = [
        ("start position", START_FEN),
        (
            "K+P v K: the pawn can promote",
            "8/8/8/4k3/8/8/4P3/4K3 w - - 0 1",
        ),
        ("K+R v K", "8/8/8/4k3/8/8/8/R3K3 w - - 0 1"),
        ("K+Q v K", "8/8/8/4k3/8/8/8/3QK3 w - - 0 1"),
        ("K+N+N v K", "8/8/8/4k3/8/8/8/1N2K1N1 w - - 0 1"),
        ("K+N v K+N", "8/8/8/4k3/8/2n5/8/4KN2 w - - 0 1"),
        ("K+B v K+N", "8/8/8/4k3/8/2n5/8/2B1K3 w - - 0 1"),
        (
            "K+B v K+B on opposite colors (c1 dark, c8 light)",
            "2b5/8/8/4k3/8/8/8/2B1K3 w - - 0 1",
        ),
        (
            "K+B+B v K, bishops on opposite colors (c1, f1)",
            "8/8/8/4k3/8/8/8/2B1KB2 w - - 0 1",
        ),
        ("K+B+N v K", "8/8/8/4k3/8/8/8/1N2KB2 w - - 0 1"),
    ];
    for (why, fen) in cases {
        for fen in both_sides_to_move(fen) {
            assert!(!parse(&fen).is_insufficient_material(), "{why}: {fen}");
        }
    }
}

// 12. the answer follows the board: the knight takes the last pawn, leaving K+N v K
#[test]
fn capturing_the_last_pawn_leaves_insufficient_material() {
    let mut pos = parse("8/8/4k3/8/4p3/8/5N2/4K3 w - - 0 1");
    assert!(!pos.is_insufficient_material());
    play(&mut pos, "f2e4");
    assert!(pos.is_insufficient_material(), "{}", pos.to_fen());
}

// ---------- 50-move rule (CE-259) ----------

// 13. 100 half-moves (50 moves each) without a capture or a pawn move are a draw: at 99 it is
//     not, one more quiet move makes it one; a FEN can also start at 100 or past it
#[test]
fn fifty_moves_without_a_capture_or_pawn_move_are_a_draw() {
    let mut pos = parse("8/8/8/4k3/8/8/8/R3K3 w - - 99 80");
    assert!(!pos.is_fifty_move_draw());
    play(&mut pos, "a1a2");
    assert!(pos.is_fifty_move_draw(), "{}", pos.to_fen());
    for clock in [100, 150] {
        let fen = format!("8/8/8/4k3/8/8/8/R3K3 w - - {clock} 80");
        assert!(parse(&fen).is_fifty_move_draw(), "{fen}");
    }
}

// 14. a pawn move or a capture on the 100th half-move resets the count: no draw
#[test]
fn a_pawn_move_or_capture_resets_the_fifty_move_count() {
    for text in ["e2e3", "a1a5"] {
        let mut pos = parse("8/8/8/r3k3/8/8/4P3/R3K3 w - - 99 80");
        play(&mut pos, text);
        assert!(!pos.is_fifty_move_draw(), "{text}: {}", pos.to_fen());
    }
}

// 15. a checkmate on the 100th half-move still wins: mate comes first. A check the king can
//     escape from does not stop the draw.
#[test]
fn checkmate_on_the_last_move_beats_the_fifty_move_rule() {
    let mut mate = parse("6k1/5ppp/8/8/8/8/8/R5K1 w - - 99 80");
    play(&mut mate, "a1a8");
    assert!(mate.is_checkmate(), "{}", mate.to_fen());
    assert!(!mate.is_fifty_move_draw(), "{}", mate.to_fen());

    // without the h7 pawn the king escapes to h7
    let mut check = parse("6k1/5pp1/8/8/8/8/8/R5K1 w - - 99 80");
    play(&mut check, "a1a8");
    assert!(check.in_check(), "{}", check.to_fen());
    assert!(check.is_fifty_move_draw(), "{}", check.to_fen());
}

// ---------- threefold repetition (CE-260) ----------

// 16. the start position counts as the first occurrence: after the knights go out and come back
//     twice, it is on the board for the third time. Nothing before that is a draw.
#[test]
fn threefold_repetition_counts_the_start_position() {
    let mut pos = parse(START_FEN);
    assert!(!pos.is_threefold_repetition());
    for round in 1..=2 {
        for text in KNIGHT_DANCE {
            play(&mut pos, text);
            let third_time = round == 2 && text == "f6g8";
            assert_eq!(
                pos.is_threefold_repetition(),
                third_time,
                "round {round}, after {text}"
            );
        }
    }
}

// 17. the same pieces with different castling rights are a different position: the kings step
//     aside and back, which ends both sides' rights. After two rounds the first position's
//     pieces stand there for the third time, but only twice without castling rights; the third
//     round makes it a draw.
#[test]
fn lost_castling_rights_make_a_different_position() {
    let mut pos = parse("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
    for round in 1..=3 {
        for text in ["e1f1", "e8f8", "f1e1", "f8e8"] {
            play(&mut pos, text);
        }
        assert_eq!(pos.is_threefold_repetition(), round == 3, "round {round}");
    }
}

// 18. after d7-d5 the position is different from the same pieces later only if White can really
//     take en passant: the en passant square is kept only then (CE-256). With the king on h4
//     exd6 is legal, so the knights need three rounds; with the king on h5 exd6 would open the
//     a5 rook's line to the king, so two rounds are enough. (The `chess` crate keeps the square
//     whenever a pawn stands next to it, so it needs three rounds in both; FIDE counts positions
//     with the same possible moves as the same position.)
#[test]
fn only_a_legal_en_passant_capture_makes_a_position_different() {
    let cases = [
        ("exd6 is legal", "1n2k3/3p4/8/r3P3/7K/8/8/1N6 b - - 0 1", 3),
        (
            "exd6 is illegal",
            "1n2k3/3p4/8/r3P2K/8/8/8/1N6 b - - 0 1",
            2,
        ),
    ];
    for (why, fen, draw_after) in cases {
        let mut pos = parse(fen);
        play(&mut pos, "d7d5");
        for round in 1..=3 {
            for text in ["b1c3", "b8c6", "c3b1", "c6b8"] {
                play(&mut pos, text);
            }
            let expected = round >= draw_after;
            assert_eq!(
                pos.is_threefold_repetition(),
                expected,
                "{why}, round {round}"
            );
        }
    }
}

// 19. unmake_move takes the position back out of the history: one move back from the third
//     occurrence it is no longer a draw, and the same move makes it one again
#[test]
fn unmake_takes_the_position_out_of_the_history() {
    let mut pos = parse(START_FEN);
    for text in KNIGHT_DANCE {
        play(&mut pos, text);
    }
    for text in &KNIGHT_DANCE[..3] {
        play(&mut pos, text);
    }
    let last = find_move(&mut pos, "f6g8");
    let undo = pos.make_move(last);
    assert!(pos.is_threefold_repetition());
    pos.unmake_move(last, undo);
    assert!(!pos.is_threefold_repetition());
    pos.make_move(last);
    assert!(pos.is_threefold_repetition());
}

// 20. the history starts at the FEN: what came before it is unknown and never counted, even
//     when the halfmove clock (8) says the knights may already have danced twice
#[test]
fn the_history_starts_at_the_fen() {
    let mut pos = parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 8 5");
    assert!(!pos.is_threefold_repetition());
    for round in 1..=2 {
        for text in KNIGHT_DANCE {
            play(&mut pos, text);
        }
        assert_eq!(pos.is_threefold_repetition(), round == 2, "round {round}");
    }
}
