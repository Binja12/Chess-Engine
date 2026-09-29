//! The contract between an engine and whatever drives it (the UCI binary, the arena, self-play):
//! every engine implements [`Engine`], so the code above it drives any engine the same way,
//! without knowing which one it is.

use std::time::Duration;

use board::moves::Move;
use board::position::Position;

/// How long a search may take when nobody gave limits: 1 second per move, for every engine (the
/// time limit of `SearchLimits::default()`).
pub const DEFAULT_TIME: Duration = Duration::from_secs(1);

/// When a search has to end. Each field is one kind of limit and `None` means no limit of that
/// kind; with several, the search ends at the first one reached. No limit at all (search until
/// stopped) is every field `None`.
///
/// `SearchLimits::default()` is what an engine uses when nobody gave it limits: 1 second per move
/// ([`DEFAULT_TIME`]), no depth or node limit. Build limits from it, e.g.
/// `SearchLimits { depth: Some(5), ..SearchLimits::default() }`: the fields left out keep their
/// default (here the time), and adding a field later does not break the code that builds them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchLimits {
    /// Search at most this many half-moves (plies) ahead.
    pub depth: Option<u32>,
    /// Look at most at this many positions (nodes).
    pub nodes: Option<u64>,
    /// Spend at most this long on the move (UCI `go movetime`).
    pub time: Option<Duration>,
}

/// The default limits: 1 second per move ([`DEFAULT_TIME`]), no depth or node limit.
impl Default for SearchLimits {
    fn default() -> SearchLimits {
        SearchLimits {
            depth: None,
            nodes: None,
            time: Some(DEFAULT_TIME),
        }
    }
}

/// A chess engine: asked about a position, it answers with its best moves or its evaluation.
/// It does not keep the game: whoever drives it holds the position and passes it in every time.
/// A position reached by playing the game's moves carries their history, so the engine can see
/// repetitions.
pub trait Engine {
    /// Sets the limits `get_moves` searches with when it is given none of its own. Until this is
    /// called, the engine uses the default limits (`SearchLimits::default()`: 1 second per move).
    fn set_limits(&mut self, limits: SearchLimits);

    /// The engine's `n` best moves in `pos`, best first: fewer when `pos` has fewer legal moves,
    /// and none when it has none (checkmate or stalemate). A game drawn by a rule (50 moves,
    /// repetition, insufficient material) still gets moves: ending the game is up to whoever
    /// runs the engine. `limits` bounds this search only; `None` searches with the limits from
    /// `set_limits`, or the default limits if it was never called.
    fn get_moves(&mut self, pos: &Position, n: usize, limits: Option<SearchLimits>) -> Vec<Move>;

    /// The engine's evaluation of `pos` in centipawns, from the side to move's point of view:
    /// +100 means the side to move is about a pawn better.
    fn get_eval(&mut self, pos: &Position) -> i32;
}
