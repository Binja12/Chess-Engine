//! # Sliding attacks: end-to-end cross-test (CE-6 "done when")
//!
//! **What is tested.** The public `board::attacks::{rook_attacks, bishop_attacks, queen_attacks}`
//! (the functions move generation calls) must return exactly what the ray-walking reference
//! `board::attack_sliders::*_attacks_ray` returns, for every square of 10,000 random boards.
//! This file sees only the crate's public API, like any caller outside `board`.
//!
//! **Why ray walking is the reference.** It is slow but simple, and it is trusted because of
//! its own unit tests in `attack_sliders.rs`: hand-computed examples (blockers, corners, edge
//! wrap) and rules that must always hold (empty-board totals 896 / 560, full board = king moves).
//!
//! **How this fits with the unit tests.** Correctness is proven in layers:
//! 1. `attack_sliders.rs` – the reference itself is right (examples + rules).
//! 2. `magic_bitboards.rs` – the magic tables equal the reference on *every* blocker pattern of
//!    every square (exhaustive), and pieces outside a square's mask never change its attacks
//!    (the mask lemma). Together these already cover every possible board, for magic.
//! 3. **This file** – the *public* functions, on *whole* boards, whatever they are built on.
//!    It does not know or care how `attacks.rs` computes the answer.
//!
//! **Swapping the implementation (PEXT).** When `attacks.rs` picks PEXT instead of magic on
//! CPUs that support it, these tests need no change: they check the public functions, so they
//! check whichever implementation is behind them. One limit: they only exercise the path
//! chosen *on the machine running the tests*. A machine without BMI2 would silently test only
//! magic, so the PEXT work must also add a test that calls the PEXT lookup directly (skipped
//! when the CPU lacks it), and layer 2 must get a PEXT counterpart.
//!
//! **Reproducing a failure.** Boards come from a fixed seed, so a failure repeats on every run.
//! The message names the piece, board number, square and occupancy (hex); copy the square and
//! occupancy into a unit test as a regression test before fixing the bug.
//!
//! Run only these tests with `cargo test -p board --test attack_tests`.

use board::attack_sliders::{bishop_attacks_ray, queen_attacks_ray, rook_attacks_ray};
use board::attacks::{bishop_attacks, queen_attacks, rook_attacks};
use board::bitboard::Bitboard;
use board::magic_bitboards::Rng;

/// How many random boards the cross-test checks (the number in the CE-6 "done when").
/// 10,000 boards x 64 squares x 3 pieces = about 1.9 million comparisons, ~0.3 s in a debug build.
const BOARDS: usize = 10_000;
/// Fixed seed: the same boards on every run, so any failure reproduces exactly.
const SEED: u64 = 0x2545_F491_4F6C_DD1D;

/// A random board whose density cycles with `i`: about 12%, 25%, 50% and 75% of the squares occupied.
/// Mixing densities matters: sparse boards test long rays that reach the edge, crowded boards
/// test rays stopped after one square. Only occupancy matters to attacks, not piece colour.
fn random_board(rng: &mut Rng, i: usize) -> Bitboard {
    let bits = match i % 4 {
        0 => rng.next_rand() & rng.next_rand() & rng.next_rand(),
        1 => rng.next_rand() & rng.next_rand(),
        2 => rng.next_rand(),
        _ => rng.next_rand() | rng.next_rand(),
    };
    Bitboard { bits }
}

/// The cross-test: on every random board, for every square, the public rook, bishop and queen
/// attacks equal the ray-walking reference. Covers the whole path a caller uses: the wrapper in
/// `attacks.rs`, the implementation behind it (magic today), and whole boards with pieces both
/// on and off each square's rays.
#[test]
fn fast_attacks_match_ray_walking_on_random_boards() {
    let mut rng = Rng::new(SEED);
    for i in 0..BOARDS {
        let occupied = random_board(&mut rng, i);
        for sq in 0..64 {
            // on failure: copy the square and board into a new regression test
            let context = format!("board {i}: square {sq}, occupied {:#018x}", occupied.bits);
            assert_eq!(
                rook_attacks(sq, occupied),
                rook_attacks_ray(sq, occupied),
                "rook, {context}"
            );
            assert_eq!(
                bishop_attacks(sq, occupied),
                bishop_attacks_ray(sq, occupied),
                "bishop, {context}"
            );
            assert_eq!(
                queen_attacks(sq, occupied),
                queen_attacks_ray(sq, occupied),
                "queen, {context}"
            );
        }
    }
}

/// Guards the cross-test itself: its boards must really range from nearly empty (at most 4
/// pieces) to crowded (at least 56). Without this, a bug in `random_board` could make the
/// cross-test check only easy boards and still pass.
#[test]
fn random_boards_cover_all_densities() {
    let mut rng = Rng::new(SEED);
    let mut min = 64;
    let mut max = 0;
    for i in 0..BOARDS {
        let count = random_board(&mut rng, i).count();
        min = min.min(count);
        max = max.max(count);
    }
    assert!(min <= 4, "sparsest board has {min} pieces");
    assert!(max >= 56, "most crowded board has {max} pieces");
}
