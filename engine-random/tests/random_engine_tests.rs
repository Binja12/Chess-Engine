//! # Random engine (CE-9, subtask CE-88)
//!
//! **What is tested.** `RandomEngine` through the `Engine` trait:
//! - it answers about the position it is given, whatever it was asked about before (it keeps no
//!   game of its own): White or Black to move, a position reached by moves, a FEN (a puzzle);
//! - no moves exactly when there is no legal move (checkmate, stalemate), and still moves when
//!   the game is drawn by a rule (50 moves, threefold repetition, insufficient material):
//!   ending the game is up to whoever runs the engine;
//! - `n` moves: none for 0, `n` different legal moves, and every legal move once when `n` is at
//!   least their number (up to `usize::MAX`); the only legal move when there is just one;
//! - every legal move comes up about equally often, and every one can come first among `n`;
//! - the same seed gives the same answers, different seeds different ones, and seed 0 works too;
//! - any limits, set with `set_limits` or passed to `get_moves`, still give a legal move: the
//!   default limits, no limits at all, and ones that leave no depth, nodes or time;
//! - `get_eval` is 0 for every position.
//!
//! An engine answering about the wrong position is caught by asking it many times: sooner or
//! later it answers with a move that is illegal in the right one.
//!
//! Every position's legal moves were also checked with the `chess` crate before this file was
//! written, and so were the checkmate, the stalemate and the threefold repetition, so a failure
//! points at the engine, not at the test. The 50-move and insufficient material positions are
//! checked by the tests themselves, with the board crate's own rules (CE-257).
//!
//! Run only these tests with `cargo test -p engine-random --test random_engine_tests`.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use board::movegen::legal_moves;
use board::moves::Move;
use board::position::{Position, START_FEN};
use engine_api::{Engine, SearchLimits};
use engine_random::RandomEngine;

/// How many times a test asks about one position.
const ASKS: usize = 100;

/// Kiwipete (perft position 2): a busy middlegame position, 48 legal moves.
const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

/// Fool's mate: after 1. f3 e5 2. g4 Qh4# White is checkmated.
const FOOLS_MATE: [&str; 4] = ["f2f3", "e7e5", "g2g4", "d8h4"];

/// Stalemate: Black's king is not in check, but the f7 queen and the g6 king cover every square
/// next to it.
const STALEMATE: &str = "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1";

/// Black has a single legal move, h8h7: the g1 rook covers the whole g-file.
const ONE_LEGAL_MOVE: &str = "7k/8/5K2/8/8/8/8/6R1 b - - 0 1";

/// 100 half-moves without a capture or a pawn move: drawn by the 50-move rule, though White has
/// 17 legal moves.
const FIFTY_MOVES: &str = "8/8/2k5/8/8/8/4R3/4K3 w - - 100 90";

/// King and bishop against king: drawn by insufficient material, though White has 13 legal
/// moves.
const KING_AND_BISHOP: &str = "8/8/2k5/8/8/8/4B3/4K3 w - - 0 1";

/// The knights go out and come back: after these 4 moves the position is the same as before.
const KNIGHT_DANCE: [&str; 4] = ["g1f3", "g8f6", "f3g1", "f6g8"];

/// Parses a FEN that the test knows is valid.
fn parse(fen: &str) -> Position {
    Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen:?} should parse, got {e:?}"))
}

/// The legal moves of `pos`, from the board crate (checked by perft and the oracle in CE-8).
fn legal(pos: &Position) -> Vec<Move> {
    legal_moves(&mut pos.clone()).as_slice().to_vec()
}

/// The moves as UCI text (e.g. `e2e4`), for failure messages.
fn texts(moves: &[Move]) -> Vec<String> {
    moves.iter().map(|mv| mv.to_string()).collect()
}

/// The position reached by playing the moves with these UCI texts from the start position. Like
/// a game's position, it carries their history.
fn play(moves: &[&str]) -> Position {
    let mut pos = parse(START_FEN);
    for text in moves {
        let mv = legal(&pos)
            .into_iter()
            .find(|mv| mv.to_string() == *text)
            .unwrap_or_else(|| panic!("{text} is not a legal move in {}", pos.to_fen()));
        pos.make_move(mv);
    }
    pos
}

/// Checks that `answer` is exactly one legal move of `pos`; `context` names the case.
fn assert_one_legal_move(answer: &[Move], pos: &Position, context: &str) {
    assert!(
        answer.len() == 1 && legal(pos).contains(&answer[0]),
        "{context}: {:?} is not one legal move of {}",
        texts(answer),
        pos.to_fen()
    );
}

// 1. the engine answers about the position it is given, whatever it was asked about before (it
//    keeps no game of its own): the start position (White to move), the position after 1. e4
//    (Black to move, reached by moves) and a FEN (a puzzle), asked about in turn
#[test]
fn answers_about_the_position_it_is_given() {
    let positions = [
        ("the start position", parse(START_FEN)),
        ("after 1. e4", play(&["e2e4"])),
        ("Kiwipete", parse(KIWIPETE)),
    ];
    let mut engine = RandomEngine::new(1);
    for _ in 0..ASKS {
        for (context, pos) in &positions {
            assert_one_legal_move(&engine.get_moves(pos, 1, None), pos, context);
        }
    }
}

// 2. no moves when the side to move has no legal move, however many are asked for: a checkmate
//    reached by moves, and a stalemate from a FEN
#[test]
fn no_moves_at_checkmate_or_stalemate() {
    let mut mated = play(&FOOLS_MATE);
    assert!(mated.is_checkmate(), "the test's own check: fool's mate");
    let mut stalemate = parse(STALEMATE);
    assert!(
        stalemate.is_stalemate(),
        "the test's own check: {STALEMATE}"
    );
    let mut engine = RandomEngine::new(2);
    for (context, pos) in [("fool's mate", &mated), ("stalemate", &stalemate)] {
        for n in [1, 5] {
            let answer = engine.get_moves(pos, n, None);
            assert!(
                answer.is_empty(),
                "{context}, {n} asked: {:?}",
                texts(&answer)
            );
        }
    }
}

// 3. a game drawn by a rule still gets a move, because the position has legal moves: the engine
//    does not end games, whoever runs it does
#[test]
fn still_moves_when_the_game_is_drawn_by_a_rule() {
    let mut dance = KNIGHT_DANCE.to_vec();
    dance.extend(KNIGHT_DANCE);
    let repeated = play(&dance);
    assert!(
        repeated.is_threefold_repetition(),
        "the test's own check: {dance:?}"
    );
    let mut fifty = parse(FIFTY_MOVES);
    assert!(
        fifty.is_fifty_move_draw(),
        "the test's own check: {FIFTY_MOVES}"
    );
    let material = parse(KING_AND_BISHOP);
    assert!(
        material.is_insufficient_material(),
        "the test's own check: {KING_AND_BISHOP}"
    );
    let mut engine = RandomEngine::new(3);
    for (context, pos) in [
        ("threefold repetition", &repeated),
        ("the 50-move rule", &fifty),
        ("insufficient material", &material),
    ] {
        for _ in 0..ASKS {
            assert_one_legal_move(&engine.get_moves(pos, 1, None), pos, context);
        }
    }
}

// 4. n moves: none when 0 are asked for, and n different legal moves when the position has more
#[test]
fn gives_n_different_legal_moves() {
    let start = parse(START_FEN);
    let allowed = legal(&start);
    let mut engine = RandomEngine::new(4);
    let none = engine.get_moves(&start, 0, None);
    assert!(none.is_empty(), "0 asked: {:?}", texts(&none));
    for _ in 0..ASKS {
        let answer = engine.get_moves(&start, 3, None);
        let different: HashSet<Move> = answer.iter().copied().collect();
        assert!(
            answer.len() == 3
                && different.len() == 3
                && answer.iter().all(|mv| allowed.contains(mv)),
            "3 asked: {:?}",
            texts(&answer)
        );
    }
}

// 5. every legal move once when n is at least their number, up to usize::MAX: the 20 start moves,
//    and the single legal move of a position that has one
#[test]
fn gives_every_legal_move_once_when_n_is_large() {
    let mut engine = RandomEngine::new(5);
    for fen in [START_FEN, ONE_LEGAL_MOVE] {
        let pos = parse(fen);
        let mut allowed = texts(&legal(&pos));
        allowed.sort();
        for n in [allowed.len(), allowed.len() + 1, 100, usize::MAX] {
            let mut answer = texts(&engine.get_moves(&pos, n, None));
            answer.sort();
            assert_eq!(answer, allowed, "{fen}, {n} asked");
        }
    }
}

// 6. with a single legal move, the engine always plays it
#[test]
fn plays_the_only_legal_move() {
    let pos = parse(ONE_LEGAL_MOVE);
    let only = legal(&pos);
    assert_eq!(only.len(), 1, "the test's own check: {ONE_LEGAL_MOVE}");
    let mut engine = RandomEngine::new(6);
    for _ in 0..ASKS {
        assert_eq!(engine.get_moves(&pos, 1, None), only);
    }
}

// 7. every legal move comes up about equally often: in 10,000 answers from the start position,
//    each of the 20 moves comes up between half and twice its fair share of 500 (a random pick
//    that leaves a move out, e.g. never the last one, fails here)
#[test]
fn every_legal_move_comes_up_about_equally_often() {
    let start = parse(START_FEN);
    let allowed = legal(&start);
    let mut engine = RandomEngine::new(7);
    let mut counts: HashMap<Move, usize> = HashMap::new();
    for _ in 0..10_000 {
        let answer = engine.get_moves(&start, 1, None);
        assert!(
            answer.len() == 1 && allowed.contains(&answer[0]),
            "{:?} is not one legal start move",
            texts(&answer)
        );
        *counts.entry(answer[0]).or_default() += 1;
    }
    for mv in &allowed {
        let count = counts.get(mv).copied().unwrap_or(0);
        assert!(
            (250..=1000).contains(&count),
            "{mv} came up {count} times in 10,000 answers"
        );
    }
}

// 8. several moves are a random pick too: in 1,000 answers of 3 moves from the start position,
//    every legal move comes first at least once (always the first moves of the list fails here)
#[test]
fn any_legal_move_can_come_first() {
    let start = parse(START_FEN);
    let allowed: HashSet<Move> = legal(&start).into_iter().collect();
    let mut engine = RandomEngine::new(8);
    let first: HashSet<Move> = (0..1000)
        .map(|_| engine.get_moves(&start, 3, None)[0])
        .collect();
    assert_eq!(first, allowed);
}

// 9. the seed decides the moves: the same seed gives the same answers, seeds 1 and 2 do not
#[test]
fn the_seed_decides_the_moves() {
    let start = parse(START_FEN);
    let answers = |seed| {
        let mut engine = RandomEngine::new(seed);
        (0..ASKS)
            .map(|_| engine.get_moves(&start, 3, None))
            .collect::<Vec<_>>()
    };
    assert_eq!(answers(9), answers(9), "the same seed twice");
    assert_ne!(answers(1), answers(2), "seeds 1 and 2");
}

// 10. seed 0 works too: every start move comes up in 1,000 answers (a xorshift generator, like
//     the board crate's, started at 0 stays at 0, and would give the same move every time)
#[test]
fn seed_zero_works() {
    let start = parse(START_FEN);
    let allowed: HashSet<Move> = legal(&start).into_iter().collect();
    let mut engine = RandomEngine::new(0);
    let played: HashSet<Move> = (0..1000)
        .flat_map(|_| engine.get_moves(&start, 1, None))
        .collect();
    assert_eq!(played, allowed);
}

// 11. limits do not stop the engine from answering: any limits, set with set_limits or passed to
//     get_moves (the default ones, none at all, even no depth, no nodes or no time), still give a
//     legal move (a UCI engine must answer every `go` with a move)
#[test]
fn any_limits_still_give_a_legal_move() {
    let cases = [
        ("the default limits", SearchLimits::default()),
        (
            "no limits at all",
            SearchLimits {
                depth: None,
                nodes: None,
                time: None,
            },
        ),
        (
            "depth 0",
            SearchLimits {
                depth: Some(0),
                ..SearchLimits::default()
            },
        ),
        (
            "0 nodes",
            SearchLimits {
                nodes: Some(0),
                ..SearchLimits::default()
            },
        ),
        (
            "no time",
            SearchLimits {
                time: Some(Duration::ZERO),
                ..SearchLimits::default()
            },
        ),
        (
            "all three at once",
            SearchLimits {
                depth: Some(1),
                nodes: Some(1),
                time: Some(Duration::from_millis(1)),
            },
        ),
    ];
    let kiwipete = parse(KIWIPETE);
    let mut engine = RandomEngine::new(11);
    for (name, limits) in cases {
        engine.set_limits(limits);
        let answer = engine.get_moves(&kiwipete, 1, None);
        assert_one_legal_move(&answer, &kiwipete, &format!("{name}, set with set_limits"));
        let answer = engine.get_moves(&kiwipete, 1, Some(limits));
        assert_one_legal_move(&answer, &kiwipete, &format!("{name}, passed to get_moves"));
    }
}

// 12. the random engine has no idea who is better: get_eval is 0 for every position, the game
//     over or not
#[test]
fn evaluates_every_position_as_0() {
    let mut engine = RandomEngine::new(12);
    for (context, pos) in [
        ("the start position", parse(START_FEN)),
        ("Kiwipete", parse(KIWIPETE)),
        ("fool's mate", play(&FOOLS_MATE)),
        ("stalemate", parse(STALEMATE)),
    ] {
        assert_eq!(engine.get_eval(&pos), 0, "{context}");
    }
}
