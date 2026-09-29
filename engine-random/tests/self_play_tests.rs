//! # Self-play (CE-9, subtask CE-88; the CE-9 "done when")
//!
//! **What is tested.** The random engine plays whole games against itself, driven the way the
//! UCI binary will drive it: the test holds the game's position, reached by playing the moves
//! (so its history shows repetitions), and before every move asks the engine for its best move
//! in it (`get_moves(&game, 1, None)`).
//! - every move is legal, and there is one whenever the game goes on; once the game is over
//!   there is none after checkmate or stalemate, and still a legal one after a draw by a rule;
//! - every game ends by a game-over rule (CE-257) within `MAX_HALF_MOVES`;
//! - over many games, every way a game can end comes up (checkmate, stalemate, 50-move rule,
//!   threefold repetition, insufficient material), so each of those rules ends some game.
//!
//! Game `n` is played by `RandomEngine::new(n)`, so a failing game replays exactly: the message
//! names it. Measured before this file was written, over 2,000 games with each of three
//! different random number generators: a random game lasts about 340 half-moves (the longest
//! about 730) and ends by insufficient material 54% of the time, by the 50-move rule 22%, in
//! checkmate 15%, in stalemate 6% and by threefold repetition 2.4%.
//!
//! Run only these tests with `cargo test -p engine-random --test self_play_tests`.

use std::collections::HashMap;

use board::movegen::legal_moves;
use board::moves::Move;
use board::position::{Position, START_FEN};
use engine_api::Engine;
use engine_random::RandomEngine;

/// How many games the "done when" test plays.
const GAMES: u64 = 100;

/// How many games the coverage test plays. The rarest ending, threefold repetition, ends about 1
/// game in 40, so 500 games all miss it with a chance of about 1 in 100,000.
const COVERAGE_GAMES: u64 = 500;

/// Longer than any game can last: the 50-move rule allows at most 100 half-moves between two
/// captures or pawn moves, and a game has only so many of those, so no game lasts more than
/// about 11,900 half-moves. A game this long means a game-over rule never fired.
const MAX_HALF_MOVES: usize = 12_000;

/// How a game ended.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Ending {
    Checkmate,
    Stalemate,
    ThreefoldRepetition,
    FiftyMoveRule,
    InsufficientMaterial,
}

/// Every way a game can end.
const ALL_ENDINGS: [Ending; 5] = [
    Ending::Checkmate,
    Ending::Stalemate,
    Ending::ThreefoldRepetition,
    Ending::FiftyMoveRule,
    Ending::InsufficientMaterial,
];

/// How the game in `pos` has ended, or `None` while it goes on. Checkmate and stalemate first (a
/// mate on the 100th half-move still wins), then the draws by a rule from the rarest to the most
/// common, so a game drawn two ways at once counts for the rarer one.
fn ending(pos: &mut Position) -> Option<Ending> {
    if pos.is_checkmate() {
        Some(Ending::Checkmate)
    } else if pos.is_stalemate() {
        Some(Ending::Stalemate)
    } else if pos.is_threefold_repetition() {
        Some(Ending::ThreefoldRepetition)
    } else if pos.is_fifty_move_draw() {
        Some(Ending::FiftyMoveRule)
    } else if pos.is_insufficient_material() {
        Some(Ending::InsufficientMaterial)
    } else {
        None
    }
}

/// The moves as UCI text (e.g. `e2e4`), for failure messages.
fn texts(moves: &[Move]) -> Vec<String> {
    moves.iter().map(|mv| mv.to_string()).collect()
}

/// Plays game `seed`: `RandomEngine::new(seed)` against itself from the start position, asked
/// for its best move in the game's position before every move, as the UCI binary will do.
/// Returns how the game ended. Panics, naming the game, on an answer that is not one legal move
/// while the game goes on, on a move after checkmate or stalemate, and on a game longer than
/// `MAX_HALF_MOVES`.
fn play_game(seed: u64) -> Ending {
    let mut game = Position::from_fen(START_FEN).expect("the start FEN is valid");
    let mut engine = RandomEngine::new(seed);
    let mut half_moves = 0;
    loop {
        // what the UCI binary will do on `go`: ask about the position it holds
        let answer = engine.get_moves(&game, 1, None);
        let legal = legal_moves(&mut game);
        let one_legal_move = answer.len() == 1 && legal.as_slice().contains(&answer[0]);

        if let Some(end) = ending(&mut game) {
            if legal.is_empty() {
                // checkmate or stalemate: no legal move, so the engine must not give one either
                assert!(
                    answer.is_empty(),
                    "game {seed}: {end:?} in {}, answer {:?}",
                    game.to_fen(),
                    texts(&answer)
                );
            } else {
                // drawn by a rule: the game is over, but the engine still has to answer
                assert!(
                    one_legal_move,
                    "game {seed}: {end:?} in {}, answer {:?} is not one legal move",
                    game.to_fen(),
                    texts(&answer)
                );
            }
            return end;
        }
        assert!(
            one_legal_move,
            "game {seed}: answer {:?} is not one legal move in {}",
            texts(&answer),
            game.to_fen()
        );
        assert!(
            half_moves < MAX_HALF_MOVES,
            "game {seed}: not over after {MAX_HALF_MOVES} half-moves, in {}",
            game.to_fen()
        );
        game.make_move(answer[0]);
        half_moves += 1;
    }
}

// 1. the CE-9 "done when": the random engine plays GAMES games against itself, each to its end,
//    without a panic, an illegal move or a missing move
#[test]
fn random_games_are_played_to_the_end() {
    for seed in 1..=GAMES {
        play_game(seed);
    }
}

// 2. the games end in every way a game can end, so every game-over rule ends some game
#[test]
fn games_end_in_every_way() {
    // the first game that ended each way
    let mut first_game: HashMap<Ending, u64> = HashMap::new();
    for seed in 1..=COVERAGE_GAMES {
        first_game.entry(play_game(seed)).or_insert(seed);
    }
    for end in ALL_ENDINGS {
        assert!(
            first_game.contains_key(&end),
            "none of {COVERAGE_GAMES} games ended by {end:?}; the first game per ending: \
             {first_game:?}"
        );
    }
}
