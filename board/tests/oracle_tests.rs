//! # Oracle comparison (CE-8, fourth subtask CE-81; part of the CE-8 "done when")
//!
//! **What is tested.** Our `legal_moves` against the `chess` crate, an independent move generator
//! used here only as a test oracle: it is a dev-dependency, compiled for tests and never part of
//! the engine (PLAN.md, section 2). Perft checks a handful of positions very deeply; this checks
//! many different positions one move deep:
//! - the harness itself on the perft reference positions (the published number of legal moves);
//! - 10,000 random positions, each reached by random legal moves from a reference position;
//! - that those random positions really exercise the special rules (en passant, castling,
//!   promotion, check, no legal moves), so that agreeing on them means something.
//!
//! Both sides are compared as sorted UCI text for the same FEN (ours from `to_fen`), so a
//! difference can also come from a wrong FEN. A failure prints the FEN and the moves only one
//! side generates.
//!
//! Run only these tests with `cargo test -p board --test oracle_tests`.

use board::color::Color;
use board::movegen::legal_moves;
use board::moves::{EN_PASSANT, KING_CASTLE, QUEEN_CASTLE};
use board::piece::PieceKind;
use board::position::{Position, START_FEN};
use std::str::FromStr;

/// How many random positions are compared (the number in the CE-8 "done when").
const RANDOM_POSITIONS: usize = 10_000;

/// The longest random walk from a start position, in half-moves.
const MAX_WALK: u64 = 30;

/// Seed of the random walks: the same positions on every run, so a failure repeats.
const SEED: u64 = 0xCE81_0AC1E;

/// Where the random walks start, with their published number of legal moves: the perft
/// reference positions (position 4 also with colors swapped).
const STARTS: [(&str, usize); 7] = [
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
        "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1",
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

/// Small deterministic random generator (xorshift64, the same algorithm as the other test
/// files; the crate's own is not public).
struct Rng {
    state: u64,
}

impl Rng {
    /// Starts a generator from `seed`, which must not be 0.
    fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// A pseudo-random number from 0 to `n - 1` (`n` must not be 0).
    fn below(&mut self, n: u64) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state % n
    }
}

/// Our legal moves in `pos`, as sorted UCI text.
fn our_moves(pos: &mut Position) -> Vec<String> {
    let mut moves: Vec<String> = legal_moves(pos)
        .as_slice()
        .iter()
        .map(|mv| mv.to_string())
        .collect();
    moves.sort();
    moves
}

/// The `chess` crate's legal moves in the position `fen`, as sorted UCI text.
fn oracle_moves(fen: &str) -> Vec<String> {
    let board = chess::Board::from_str(fen)
        .unwrap_or_else(|e| panic!("the chess crate rejects {fen}: {e:?}"));
    let mut moves: Vec<String> = chess::MoveGen::new_legal(&board)
        .map(|mv| mv.to_string())
        .collect();
    moves.sort();
    moves
}

/// Checks that our legal moves in `pos` equal the oracle's for the same FEN. On a difference
/// it panics with `context`, the FEN, and the moves only one side generates.
fn assert_same_moves(pos: &mut Position, context: &str) {
    let fen = pos.to_fen();
    let ours = our_moves(pos);
    let oracle = oracle_moves(&fen);
    if ours != oracle {
        let only_ours: Vec<&String> = ours.iter().filter(|mv| !oracle.contains(mv)).collect();
        let only_oracle: Vec<&String> = oracle.iter().filter(|mv| !ours.contains(mv)).collect();
        panic!(
            "{context}: {fen}\n  only ours: {only_ours:?}\n  only the oracle: {only_oracle:?}\n  \
             moves: {} ours, {} the oracle's",
            ours.len(),
            oracle.len()
        );
    }
}

/// Plays up to `length` random legal moves from `pos`, stopping early if the side to move has
/// none (checkmate or stalemate).
fn random_walk(pos: &mut Position, length: u64, rng: &mut Rng) {
    for _ in 0..length {
        let moves = legal_moves(pos);
        if moves.is_empty() {
            return;
        }
        let mv = moves.as_slice()[rng.below(moves.len() as u64) as usize];
        pos.make_move(mv);
    }
}

/// Builds the random positions, the same ones on every call, and hands each one with its
/// number to `visit`: position `i` starts from `STARTS[i % 7]` and walks 0 to `MAX_WALK` random
/// legal moves.
fn for_each_random_position(mut visit: impl FnMut(usize, &mut Position)) {
    let mut rng = Rng::new(SEED);
    for i in 0..RANDOM_POSITIONS {
        let (fen, _) = STARTS[i % STARTS.len()];
        let mut pos = Position::from_fen(fen).expect("reference FENs are valid");
        let length = rng.below(MAX_WALK + 1);
        random_walk(&mut pos, length, &mut rng);
        visit(i, &mut pos);
    }
}

// 1. the harness on the reference positions: the oracle finds the published number of legal
//    moves, and exactly the same moves as we do
#[test]
fn oracle_agrees_on_the_reference_positions() {
    for (fen, count) in STARTS {
        assert_eq!(oracle_moves(fen).len(), count, "oracle, {fen}");
        let mut pos = Position::from_fen(fen).expect("reference FENs are valid");
        assert_same_moves(&mut pos, "reference position");
    }
}

// 2. the CE-8 "done when": the same legal moves as the oracle on 10,000 random positions
#[test]
fn legal_moves_match_the_oracle_on_10000_random_positions() {
    for_each_random_position(|i, pos| assert_same_moves(pos, &format!("random position {i}")));
}

// 3. the random positions really exercise the special rules, so agreeing on them means
//    something: each rule shows up in many positions. The fixed seed gives en passant 65,
//    castling 1,368, promotion 1,019, check 518 and no legal moves 91; the minimums are about
//    half of that, so a change to the walk that stops reaching a rule fails here
#[test]
fn random_positions_cover_the_special_rules() {
    let (mut en_passant, mut castling, mut promotion, mut check, mut no_moves) = (0, 0, 0, 0, 0);
    for_each_random_position(|_, pos| {
        let us = pos.side_to_move();
        let them = match us {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        if pos.is_attacked(pos.pieces(us, PieceKind::King).lsb(), them) {
            check += 1;
        }
        let moves = legal_moves(pos);
        let moves = moves.as_slice();
        if moves.is_empty() {
            no_moves += 1;
        }
        if moves.iter().any(|mv| mv.flag() == EN_PASSANT) {
            en_passant += 1;
        }
        if moves
            .iter()
            .any(|mv| mv.flag() == KING_CASTLE || mv.flag() == QUEEN_CASTLE)
        {
            castling += 1;
        }
        if moves.iter().any(|mv| mv.promotion().is_some()) {
            promotion += 1;
        }
    });
    println!(
        "en passant {en_passant}, castling {castling}, promotion {promotion}, check {check}, \
         no legal moves {no_moves}"
    );
    assert!(
        en_passant >= 30,
        "positions with a legal en passant capture: {en_passant}"
    );
    assert!(
        castling >= 600,
        "positions with a legal castling move: {castling}"
    );
    assert!(
        promotion >= 500,
        "positions with a legal promotion: {promotion}"
    );
    assert!(
        check >= 250,
        "positions where the side to move is in check: {check}"
    );
    assert!(no_moves >= 40, "positions with no legal moves: {no_moves}");
}
