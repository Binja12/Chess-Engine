//! The contract between an engine and whatever drives it (the UCI binary, the arena, self-play).
//! For now [`SearchLimits`]: when a search has to end.

use std::time::Duration;

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
