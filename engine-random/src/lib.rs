//! `RandomEngine`: plays random legal moves, every legal move with the same chance. It knows the
//! rules and nothing else, which makes it the simplest engine that can play whole games: it
//! tests the pipeline (engine trait, UCI binary, Lichess) before any search exists.

// the seeded random number generator and the random pick, in rng.rs
mod rng;

use board::position::Position;
use board::{movegen::legal_moves, moves::Move};
use engine_api::{Engine, SearchLimits};
use rng::Rng;

/// An engine that picks uniformly random legal moves, drawn from its own seeded random number
/// generator: the same seed always gives the same moves, so a game can be replayed exactly.
pub struct RandomEngine {
    /// Where the random picks come from.
    rng: Rng,
    /// The limits from `set_limits`, the default ones until it is called.
    limits: SearchLimits,
}

impl RandomEngine {
    /// A new engine with the default limits, its random numbers starting from `seed`. Every seed
    /// works, 0 too.
    pub fn new(seed: u64) -> RandomEngine {
        RandomEngine {
            rng: Rng::new(seed),
            limits: SearchLimits::default(),
        }
    }
}

impl Engine for RandomEngine {
    /// Keeps `limits` for the searches that get none of their own.
    fn set_limits(&mut self, limits: SearchLimits) {
        self.limits = limits;
    }

    /// `n` different legal moves of `pos`, picked at random: every legal move has the same chance
    /// to be picked, and to come first. All of them, in random order, when `pos` has at most `n`;
    /// none when it has none. The limits do not matter: picking takes no time.
    fn get_moves(&mut self, pos: &Position, n: usize, limits: Option<SearchLimits>) -> Vec<Move> {
        // the limits this search would use: the ones passed in, or else the kept ones
        let _limits = limits.unwrap_or(self.limits);
        // `legal_moves` plays each move to test it, so it needs a copy it may change
        let moves = legal_moves(&mut pos.clone()).as_slice().to_vec();
        self.rng.pick(moves, n)
    }

    /// Always 0: the random engine has no idea who is better.
    fn get_eval(&mut self, _pos: &Position) -> i32 {
        0
    }
}
