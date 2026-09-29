//! # Search limits (CE-9, subtask CE-87)
//!
//! **What is tested.** The default limits, `SearchLimits::default()`, which every engine uses
//! when nobody gives it limits: 1 second per move and no depth or node limit, so an engine asked
//! without limits never searches forever.
//!
//! Run only these tests with `cargo test -p engine-api --test search_limits_tests`.

use std::time::Duration;

use engine_api::{DEFAULT_TIME, SearchLimits};

// 1. the default limits, for when nobody gives any: 1 second per move, no depth or node limit
#[test]
fn the_default_is_one_second_per_move() {
    assert_eq!(DEFAULT_TIME, Duration::from_secs(1));
    assert_eq!(
        SearchLimits::default(),
        SearchLimits {
            depth: None,
            nodes: None,
            time: Some(DEFAULT_TIME),
        }
    );
}
