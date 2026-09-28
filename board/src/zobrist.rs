//! Zobrist hashing keys: one random 64-bit key per position feature (a piece on a square, the
//! castling rights, the en passant file, Black to move). A position's hash is the XOR of the
//! keys of its features, so a move updates it with a few XORs (`h ^ k ^ k == h`).
//! Keys come from a fixed seed, so every run uses the same keys and hashes are reproducible.

use crate::color::Color;
use crate::magic_bitboards::Rng;
use crate::piece::PieceKind;
use std::sync::LazyLock;

/// Seed for the key generator. Changing it changes every hash.
const SEED: u64 = 0x2D35_8DCC_AA6C_78A5;

/// Every Zobrist key: 768 piece keys + 16 castling + 8 en passant + 1 side to move = 793.
struct ZobristKeys {
    /// `pieces[color][kind][sq]`
    pieces: [[[u64; 64]; 6]; 2],
    /// `castling[rights]`: one key per combination of the 4 castling flags.
    castling: [u64; 16],
    /// `en_passant[file]`
    en_passant: [u64; 8],
    /// XORed in when Black is to move.
    black_to_move: u64,
}

/// The keys, built on first use.
static KEYS: LazyLock<ZobristKeys> = LazyLock::new(generate_keys);

/// Fills every key from `Rng::new(SEED)` with `next_rand_star`, in a fixed order:
/// pieces (color, kind, square), then castling, en passant, black to move.
fn generate_keys() -> ZobristKeys {
    let mut pieces = [[[0u64; 64]; 6]; 2];
    let mut rng = Rng::new(SEED);
    for by_kind in pieces.iter_mut() {
        for by_square in by_kind.iter_mut() {
            for key in by_square.iter_mut() {
                *key = rng.next_rand_star();
            }
        }
    }
    let mut castling = [0u64; 16];
    for key in castling.iter_mut() {
        *key = rng.next_rand_star();
    }
    let mut en_passant = [0u64; 8];
    for key in en_passant.iter_mut() {
        *key = rng.next_rand_star();
    }
    let black_to_move = rng.next_rand_star();
    ZobristKeys {
        pieces,
        castling,
        en_passant,
        black_to_move,
    }
}

/// The key for a piece of this color and kind standing on `sq`.
pub fn piece_key(color: Color, kind: PieceKind, sq: u8) -> u64 {
    KEYS.pieces[color as usize][kind as usize][sq as usize]
}

/// The key for a set of castling rights (an OR of the castling flags, 0..=15).
pub fn castling_key(rights: u8) -> u64 {
    KEYS.castling[rights as usize]
}

/// The key for an en passant square on `file` (0 = a … 7 = h).
pub fn en_passant_key(file: u8) -> u64 {
    KEYS.en_passant[file as usize]
}

/// The key XORed in when Black is to move.
pub fn black_to_move_key() -> u64 {
    KEYS.black_to_move
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Every key in one list, in generation order.
    fn all_keys(keys: &ZobristKeys) -> Vec<u64> {
        let mut all = Vec::new();
        for color in &keys.pieces {
            for kind in color {
                all.extend_from_slice(kind);
            }
        }
        all.extend_from_slice(&keys.castling);
        all.extend_from_slice(&keys.en_passant);
        all.push(keys.black_to_move);
        all
    }

    // 1. fixed seed: the same keys on every run
    #[test]
    fn keys_are_the_same_on_every_run() {
        assert_eq!(all_keys(&generate_keys()), all_keys(&generate_keys()));
    }

    // 2. no key is 0 (it would not change the hash) and no two keys are equal
    #[test]
    fn keys_are_nonzero_and_distinct() {
        let all = all_keys(&KEYS);
        assert!(all.iter().all(|&k| k != 0), "a key is 0");
        let distinct: HashSet<u64> = all.iter().copied().collect();
        assert_eq!(distinct.len(), all.len(), "two keys are equal");
    }

    // 3. no small XOR relation: a ^ b is never another key (3 keys) and never
    //    another pair's c ^ d (4 keys). ~314,000 pairs.
    #[test]
    fn no_small_xor_relations_between_keys() {
        let all = all_keys(&KEYS);
        let singles: HashSet<u64> = all.iter().copied().collect();
        let mut pairs = HashSet::new();
        for i in 0..all.len() {
            for j in i + 1..all.len() {
                let x = all[i] ^ all[j];
                assert!(!singles.contains(&x), "key {i} ^ key {j} is another key");
                assert!(pairs.insert(x), "key {i} ^ key {j} equals another pair");
            }
        }
    }

    // 4. the accessors return the matching table entry
    #[test]
    fn accessors_read_the_table() {
        assert_eq!(
            piece_key(Color::Black, PieceKind::Queen, 59),
            KEYS.pieces[1][4][59]
        );
        assert_eq!(castling_key(9), KEYS.castling[9]);
        assert_eq!(en_passant_key(4), KEYS.en_passant[4]);
        assert_eq!(black_to_move_key(), KEYS.black_to_move);
    }
}
