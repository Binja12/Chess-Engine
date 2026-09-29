//! # Make / unmake (CE-7, fourth subtask, and the CE-7 "done when")
//!
//! **What is tested.** `Position::make_move` and `unmake_move` through the public API:
//! - every move kind gives the expected position (compared as FEN, so a failure prints a board),
//!   and unmake gives back exactly the original position, hash included;
//! - castling rights: king moves, rook moves, a rook captured on its corner, a rook that leaves
//!   and comes back;
//! - move counters, side to move, and the en passant square (set only when an en passant capture
//!   is legal, Stockfish's rule, the same one `from_fen` uses);
//! - the hash: transpositions hash equal, and after every move the incrementally kept hash
//!   equals a from-scratch recomputation and the hash of the same position parsed from its FEN
//!   (after a move that left the mover's king attacked, that FEN must be rejected instead);
//! - the CE-7 "done when": 10,000 random move sequences, each taken back move by move, restore
//!   the position exactly.
//!
//! Run only these tests with `cargo test -p board --test make_move_tests`.

use board::bitboard::Bitboard;
use board::color::Color;
use board::movegen::{MoveGen, generate_moves};
use board::moves::Move;
use board::piece::PieceKind;
use board::position::{FenError, Position, START_FEN, Undo};

/// Small deterministic random generator for the random sequences (xorshift64, the same algorithm
/// the crate uses internally, which is not public). A fixed seed gives the same sequences on
/// every run, so a failure repeats.
struct Rng {
    state: u64,
}

impl Rng {
    /// Starts a generator from `seed`, which must not be 0.
    fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// The next pseudo-random 64-bit number.
    fn next_rand(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }
}

/// Where the random sequences start: the start position, Kiwipete (both sides to move), perft
/// positions 3-6, and positions made for castling, en passant and promotions.
const START_POSITIONS: &[&str] = &[
    START_FEN,
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
    "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
    "4k3/8/8/2PpP3/8/8/8/4K3 w - d6 0 1",
    "1r2k3/P7/8/8/8/8/8/4K3 w - - 0 1",
    "r3k2r/8/8/8/8/8/1p6/R3K2R b KQkq - 0 1",
];

const COLORS: [Color; 2] = [Color::White, Color::Black];
const KINDS: [PieceKind; 6] = [
    PieceKind::Pawn,
    PieceKind::Knight,
    PieceKind::Bishop,
    PieceKind::Rook,
    PieceKind::Queen,
    PieceKind::King,
];

/// Parses a FEN that the test knows is valid.
fn parse(fen: &str) -> Position {
    Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen:?} should parse, got {e:?}"))
}

/// The other side.
fn other(color: Color) -> Color {
    match color {
        Color::White => Color::Black,
        Color::Black => Color::White,
    }
}

/// The move with this UCI text (e.g. `"e1g1"`, `"a7a8q"`), taken from `generate_moves` so its
/// flag is the real one. Panics if the position has no such move.
fn find_move(pos: &Position, text: &str) -> Move {
    generate_moves(pos, MoveGen::ALL)
        .as_slice()
        .iter()
        .copied()
        .find(|mv| mv.to_string() == text)
        .unwrap_or_else(|| panic!("{text} is not a move in {}", pos.to_fen()))
}

/// Checks everything `make_move` keeps up to date incrementally: the hash equals a from-scratch
/// recomputation and the hash of the same position parsed from its FEN, and the 12 bitboards,
/// the color sets and the mailbox agree. After a pseudo-legal move that left the mover's own
/// king attacked, the side to move could capture that king, so `from_fen` must reject the FEN
/// instead.
fn assert_consistent(pos: &Position) {
    let fen = pos.to_fen();
    assert_eq!(pos.hash(), pos.compute_hash(), "hash vs recomputed: {fen}");
    let mover = other(pos.side_to_move());
    let mover_king = pos.pieces(mover, PieceKind::King).lsb();
    if pos.is_attacked(mover_king, pos.side_to_move()) {
        assert_eq!(
            Position::from_fen(&fen).err(),
            Some(FenError::KingCanBeCaptured),
            "illegal move, FEN must be rejected: {fen}"
        );
    } else {
        assert_eq!(pos.hash(), parse(&fen).hash(), "hash vs FEN: {fen}");
    }

    let mut all = Bitboard::EMPTY;
    for color in COLORS {
        let mut own = Bitboard::EMPTY;
        for kind in KINDS {
            own |= pos.pieces(color, kind);
        }
        assert_eq!(own, pos.color_pieces(color), "{color:?} pieces: {fen}");
        all |= own;
    }
    assert_eq!(all, pos.occupied(), "occupied: {fen}");
    for sq in 0..64 {
        match pos.piece_at(sq) {
            Some(p) => assert!(
                pos.pieces(p.color, p.kind).contains(sq),
                "square {sq}: {fen}"
            ),
            None => assert!(!pos.occupied().contains(sq), "square {sq}: {fen}"),
        }
    }
}

/// Plays `text` in `fen` and checks the result is `expected` (as FEN) and consistent; then
/// takes it back and checks the position is exactly the original again.
fn assert_make_unmake(fen: &str, text: &str, expected: &str) {
    let mut pos = parse(fen);
    let original = pos.clone();
    let mv = find_move(&pos, text);
    let undo = pos.make_move(mv);
    assert_eq!(pos.to_fen(), expected, "{text} in {fen}");
    assert_consistent(&pos);
    pos.unmake_move(mv, undo);
    assert_eq!(pos, original, "unmake {text} in {fen}");
}

/// Plays the moves (UCI text) one after another, checking consistency after each; returns what
/// `unplay` needs to take them back.
fn play(pos: &mut Position, moves: &[&str]) -> Vec<(Move, Undo)> {
    let mut played = Vec::new();
    for text in moves {
        let mv = find_move(pos, text);
        let undo = pos.make_move(mv);
        assert_consistent(pos);
        played.push((mv, undo));
    }
    played
}

/// Takes back moves returned by `play`, the last one first.
fn unplay(pos: &mut Position, played: Vec<(Move, Undo)>) {
    for (mv, undo) in played.into_iter().rev() {
        pos.unmake_move(mv, undo);
    }
}

// ---------- each move kind: make gives the expected position, unmake the original ----------

// 1. a quiet move: the piece moves, the turn passes, the halfmove clock counts up
#[test]
fn quiet_move() {
    assert_make_unmake(
        START_FEN,
        "g1f3",
        "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq - 1 1",
    );
}

// 2. a double push sets the en passant square only when an enemy pawn could capture onto it
#[test]
fn double_push_sets_en_passant_only_when_capturable() {
    // no black pawn beside e4: no en passant square
    assert_make_unmake(
        START_FEN,
        "e2e4",
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
    );
    // the black pawn on d4 could take on e3
    assert_make_unmake(
        "rnbqkbnr/ppp1pppp/8/8/3p4/8/PPPPPPPP/RNBQKBNR w KQkq - 0 3",
        "e2e4",
        "rnbqkbnr/ppp1pppp/8/8/3pP3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 3",
    );
    // Black's double push: the white pawn on e5 could take on d6
    assert_make_unmake(
        "rnbqkbnr/pppppppp/8/4P3/8/8/PPPP1PPP/RNBQKBNR b KQkq - 0 2",
        "d7d5",
        "rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3",
    );
    // the a5 pawn reaches h3 only by wrapping around the board edge (h3 = 23, 23 + 9 = 32 = a5),
    // so it cannot take on h3
    assert_make_unmake(
        "4k3/8/8/p7/8/8/7P/4K3 w - - 0 1",
        "h2h4",
        "4k3/8/8/p7/7P/8/8/4K3 b - - 0 1",
    );
}

// 2b. ... and only when at least one en passant capture is legal (Stockfish's rule, the same
//     one from_fen uses, so the position hashes the same as its FEN)
#[test]
fn double_push_sets_en_passant_only_when_a_capture_is_legal() {
    // e7-e5: the d5 and f5 pawns could take on e6, but both are pinned to the e4 king
    assert_make_unmake(
        "4k3/4p3/2b3b1/3P1P2/4K3/8/8/8 b - - 0 1",
        "e7e5",
        "4k3/8/2b3b1/3PpP2/4K3/8/8/8 w - - 0 2",
    );
    // perft position 3, e2-e4: fxe3 would open the b4 rook's line to the h4 king
    assert_make_unmake(
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "e2e4",
        "8/2p5/3p4/KP5r/1R2Pp1k/8/6P1/8 b - - 0 1",
    );
    // e2-e4 uncovers the d1 bishop's check on h5, which fxe3 does not block
    assert_make_unmake(
        "8/8/8/7k/5p2/8/4P3/K2B4 w - - 0 1",
        "e2e4",
        "8/8/8/7k/4Pp2/8/8/K2B4 b - - 0 1",
    );
    // one legal capture is enough: d4 is pinned on the d-file, f4 is free
    assert_make_unmake(
        "3k4/8/8/8/3p1p2/8/4P3/3R3K w - - 0 1",
        "e2e4",
        "3k4/8/8/8/3pPp2/8/8/3R3K b - e3 0 1",
    );
}

// 3. a capture removes the captured piece and resets the halfmove clock
#[test]
fn captures() {
    assert_make_unmake(
        "4k3/8/8/3p4/4P3/8/8/4K3 w - - 5 10",
        "e4d5",
        "4k3/8/8/3P4/8/8/8/4K3 b - - 0 10",
    );
    assert_make_unmake(
        "4k3/8/8/3p4/8/4N3/8/4K3 w - - 5 10",
        "e3d5",
        "4k3/8/8/3N4/8/8/8/4K3 b - - 0 10",
    );
}

// 4. en passant removes the pawn beside the to square (d5 for d6), not a piece on it
#[test]
fn en_passant_captures() {
    assert_make_unmake(
        "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2",
        "e5d6",
        "4k3/8/3P4/8/8/8/8/4K3 b - - 0 2",
    );
    assert_make_unmake(
        "4k3/8/8/8/3Pp3/8/8/4K3 b - d3 0 2",
        "e4d3",
        "4k3/8/8/8/8/3p4/8/4K3 w - - 0 3",
    );
}

// 5. a promotion replaces the pawn with the chosen piece; unmake brings back the pawn
#[test]
fn promotions() {
    for (letter, piece) in [('q', 'Q'), ('r', 'R'), ('b', 'B'), ('n', 'N')] {
        assert_make_unmake(
            "4k3/P7/8/8/8/8/8/4K3 w - - 3 40",
            &format!("a7a8{letter}"),
            &format!("{piece}3k3/8/8/8/8/8/8/4K3 b - - 0 40"),
        );
    }
    // promotion with a capture
    assert_make_unmake(
        "1r2k3/P7/8/8/8/8/8/4K3 w - - 0 40",
        "a7b8q",
        "1Q2k3/8/8/8/8/8/8/4K3 b - - 0 40",
    );
    // Black promotes on rank 1
    assert_make_unmake(
        "4k3/8/8/8/8/8/p7/4K3 b - - 0 40",
        "a2a1q",
        "4k3/8/8/8/8/8/8/q3K3 w - - 0 41",
    );
}

// 6. castling moves the king and the rook, and clears that side's rights
#[test]
fn castling_moves_king_and_rook() {
    let white = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
    assert_make_unmake(white, "e1g1", "r3k2r/8/8/8/8/8/8/R4RK1 b kq - 1 1");
    assert_make_unmake(white, "e1c1", "r3k2r/8/8/8/8/8/8/2KR3R b kq - 1 1");
    let black = "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1";
    assert_make_unmake(black, "e8g8", "r4rk1/8/8/8/8/8/8/R3K2R w KQ - 1 2");
    assert_make_unmake(black, "e8c8", "2kr3r/8/8/8/8/8/8/R3K2R w KQ - 1 2");
}

// ---------- castling rights ----------

// 7. a king move ends both of its side's rights; a rook move ends only its own
#[test]
fn king_and_rook_moves_clear_castling_rights() {
    let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
    assert_make_unmake(fen, "e1f1", "r3k2r/8/8/8/8/8/8/R4K1R b kq - 1 1");
    assert_make_unmake(fen, "h1g1", "r3k2r/8/8/8/8/8/8/R3K1R1 b Qkq - 1 1");
    assert_make_unmake(fen, "a1b1", "r3k2r/8/8/8/8/8/8/1R2K2R b Kkq - 1 1");
}

// 8. capturing a rook on its corner ends the victim's right
#[test]
fn capturing_a_rook_on_its_corner_clears_its_right() {
    // Rxh8: the white rook leaves h1 (K gone) and takes the h8 rook (k gone)
    assert_make_unmake(
        "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
        "h1h8",
        "r3k2R/8/8/8/8/8/8/R3K3 b Qq - 0 1",
    );
    // Bxa8
    assert_make_unmake(
        "r3k2r/8/8/8/8/8/6B1/R3K2R w KQkq - 0 1",
        "g2a8",
        "B3k2r/8/8/8/8/8/8/R3K2R b KQk - 0 1",
    );
    // bxa8=Q
    assert_make_unmake(
        "r3k2r/1P6/8/8/8/8/8/R3K2R w KQkq - 0 1",
        "b7a8q",
        "Q3k2r/8/8/8/8/8/8/R3K2R b KQk - 0 1",
    );
    // Black: bxa1=Q takes the a1 rook
    assert_make_unmake(
        "r3k2r/8/8/8/8/8/1p6/R3K2R b KQkq - 0 1",
        "b2a1q",
        "r3k2r/8/8/8/8/8/8/q3K2R w Kkq - 0 2",
    );
}

// 9. a rook that leaves its corner and comes back does not get its right back
#[test]
fn a_returning_rook_does_not_regain_its_right() {
    let mut pos = parse("r3k2r/7p/8/8/8/8/8/R3K2R w KQkq - 0 1");
    let original = pos.clone();
    let played = play(&mut pos, &["h1h2", "h7h6", "h2h1"]);
    assert_eq!(pos.to_fen(), "r3k2r/8/7p/8/8/8/8/R3K2R b Qkq - 1 2");
    unplay(&mut pos, played);
    assert_eq!(pos, original);
}

// ---------- counters, turn and en passant square ----------

// 10. halfmove clock: +1 after a quiet piece move, 0 after a pawn move; fullmove +1 after Black
#[test]
fn move_counters() {
    assert_make_unmake(
        "4k3/8/8/8/8/8/8/4K1N1 w - - 5 20",
        "g1f3",
        "4k3/8/8/8/8/5N2/8/4K3 b - - 6 20",
    );
    assert_make_unmake(
        "4k3/8/8/8/8/8/4P3/4K3 w - - 5 20",
        "e2e3",
        "4k3/8/8/8/8/4P3/8/4K3 b - - 0 20",
    );
    assert_make_unmake(
        "4k3/8/8/8/8/8/8/4K3 b - - 5 20",
        "e8d8",
        "3k4/8/8/8/8/8/8/4K3 w - - 6 21",
    );
}

// 11. any move except the en passant capture clears the en passant square
#[test]
fn en_passant_square_is_cleared_by_the_next_move() {
    assert_make_unmake(
        "rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3",
        "g1f3",
        "rnbqkbnr/ppp1pppp/8/3pP3/8/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 3",
    );
}

// ---------- hash ----------

// 12. two move orders that reach the same position give the same hash
#[test]
fn transpositions_hash_equal() {
    let mut a = parse(START_FEN);
    play(&mut a, &["g1f3", "g8f6", "b1c3"]);
    let mut b = parse(START_FEN);
    play(&mut b, &["b1c3", "g8f6", "g1f3"]);
    assert_eq!(a.to_fen(), b.to_fen());
    assert_eq!(a.hash(), b.hash());
    assert_ne!(a.hash(), parse(START_FEN).hash());
}

// ---------- every move, and the CE-7 "done when" ----------

// 13. every pseudo-legal move of every start position keeps the position consistent, and
//     unmake restores it exactly (including moves that leave the own king in check)
#[test]
fn every_move_of_the_start_positions_unmakes_exactly() {
    for fen in START_POSITIONS {
        let mut pos = parse(fen);
        let original = pos.clone();
        for &mv in generate_moves(&original, MoveGen::ALL).as_slice() {
            let undo = pos.make_move(mv);
            assert_consistent(&pos);
            pos.unmake_move(mv, undo);
            assert_eq!(pos, original, "{mv} in {fen}");
        }
    }
}

/// Plays up to `depth` random legal moves from `pos`, checking consistency after every move and
/// that every unmake restores the position exactly. A move that leaves the mover's own king in
/// check is taken back and another one is tried, so no king is ever captured.
fn random_walk(pos: &mut Position, depth: usize, rng: &mut Rng) {
    if depth == 0 {
        return;
    }
    let moves = generate_moves(pos, MoveGen::ALL);
    let moves = moves.as_slice();
    if moves.is_empty() {
        return;
    }
    // try the moves in order, starting at a random one, until one is legal
    let first = (rng.next_rand() % moves.len() as u64) as usize;
    for i in 0..moves.len() {
        let mv = moves[(first + i) % moves.len()];
        let before = pos.clone();
        let mover = pos.side_to_move();
        let undo = pos.make_move(mv);
        let king = pos.pieces(mover, PieceKind::King).lsb();
        if pos.is_attacked(king, other(mover)) {
            pos.unmake_move(mv, undo);
            assert_eq!(
                *pos,
                before,
                "unmake of illegal {mv} in {}",
                before.to_fen()
            );
            continue;
        }
        assert_consistent(pos);
        random_walk(pos, depth - 1, rng);
        pos.unmake_move(mv, undo);
        assert_eq!(*pos, before, "unmake {mv} in {}", before.to_fen());
        return;
    }
}

// 14. CE-7 "done when": make then unmake restores position and hash on 10,000 random sequences
//     (fixed seed, so a failure repeats on every run)
#[test]
fn make_unmake_round_trips_on_10000_random_sequences() {
    let mut rng = Rng::new(0xCE07_5EED);
    for i in 0..10_000 {
        let fen = START_POSITIONS[i % START_POSITIONS.len()];
        let mut pos = parse(fen);
        let depth = 1 + (rng.next_rand() % 12) as usize;
        random_walk(&mut pos, depth, &mut rng);
        assert_eq!(pos, parse(fen), "sequence {i} from {fen}");
    }
}
