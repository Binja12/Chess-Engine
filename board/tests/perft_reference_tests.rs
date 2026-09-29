//! # Perft reference positions (CE-8, third subtask CE-79; the CE-8 "done when")
//!
//! **What is tested.** `perft` on the standard reference positions against their published
//! counts at depths 1 to 5 (<https://www.chessprogramming.org/Perft_Results>). Between them the
//! positions cover castling (through and out of check), en passant (including the pin along the
//! rank in position 3), promotions with and without capture, pins, checks and double checks.
//! Position 4 is also tested with the colors swapped: the same counts prove White and Black are
//! handled alike.
//!
//! The table is split by size so that `cargo test` stays fast:
//! - counts up to [`QUICK_LIMIT`] run every time (under a second in a debug build);
//! - the larger ones, up to about 190 million positions each, are an ignored test. Run them
//!   before a merge with
//!   `cargo test --release -p board --test perft_reference_tests -- --ignored`
//!   (about 10-20 s in release; a debug build is roughly 10 times slower).

use board::perft::perft;
use board::position::{Position, START_FEN};

/// Counts up to this size run in every `cargo test`; larger ones are in the ignored test.
const QUICK_LIMIT: u64 = 1_000_000;

/// The reference positions: name, FEN, and the published perft counts at depths 1 to 5.
const REFERENCE: [(&str, &str, [u64; 5]); 7] = [
    ("start", START_FEN, [20, 400, 8_902, 197_281, 4_865_609]),
    (
        "Kiwipete",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        [48, 2_039, 97_862, 4_085_603, 193_690_690],
    ),
    (
        "position 3",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        [14, 191, 2_812, 43_238, 674_624],
    ),
    (
        "position 4",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        [6, 264, 9_467, 422_333, 15_833_292],
    ),
    (
        "position 4, colors swapped",
        "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1",
        [6, 264, 9_467, 422_333, 15_833_292],
    ),
    (
        "position 5",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        [44, 1_486, 62_379, 2_103_487, 89_941_194],
    ),
    (
        "position 6",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        [46, 2_079, 89_890, 3_894_594, 164_075_551],
    ),
];

/// Runs perft on every reference position at every depth whose published count passes `keep`,
/// and checks it against that count.
fn check_reference_counts(keep: fn(u64) -> bool) {
    for (name, fen, counts) in REFERENCE {
        let mut pos = Position::from_fen(fen).expect("reference FENs are valid");
        for (i, &count) in counts.iter().enumerate() {
            if keep(count) {
                let depth = i as u32 + 1;
                assert_eq!(perft(&mut pos, depth), count, "{name}, depth {depth}");
            }
        }
    }
}

// 1. every published count up to QUICK_LIMIT: all positions to depth 3, most to depth 4,
//    position 3 to depth 5
#[test]
fn reference_positions_match_the_published_counts_quick() {
    check_reference_counts(|count| count <= QUICK_LIMIT);
}

// 2. the rest, up to depth 5 everywhere: together with test 1 the whole table, the CE-8
//    "done when"
#[test]
#[ignore = "slow in debug builds: cargo test --release -p board --test perft_reference_tests -- --ignored"]
fn reference_positions_match_the_published_counts_to_depth_5() {
    check_reference_counts(|count| count > QUICK_LIMIT);
}
