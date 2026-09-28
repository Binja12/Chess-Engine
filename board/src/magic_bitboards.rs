//! Magic bitboards: find, for every square, a multiplier (`magic`) that turns the blocker
//! pattern on that square's mask into a small table index:
//! `index = ((occupied & mask) * magic) >> (64 - mask bits)`.
//! Magics are found by guess-and-check against every blocker pattern, using the ray-walking
//! attacks from `attack_sliders` as the source of truth.

use crate::attack_sliders::*;
use crate::bitboard::Bitboard;
use crate::masks::{FILE_A, FILE_H, INNER_MASK, RANK_1, RANK_8, file_mask, rank_mask};
use std::sync::LazyLock;

/// Which sliding piece a mask or magic belongs to. The queen has none: it is rook | bishop.
#[derive(Clone, Copy, Debug, PartialEq)]

pub enum Slider {
    Rook,
    Bishop,
}

/// Slots in the rook attack table: the sum of 2^(mask bits) over all 64 squares.
pub const ROOK_TABLE_SIZE: usize = 102_400;
/// Slots in the bishop attack table: the sum of 2^(mask bits) over all 64 squares.
pub const BISHOP_TABLE_SIZE: usize = 5_248;

/// Rook magic for every square, found by `find_magic` from seed `0x9E37_79B9_7F4A_7C15`.
/// Regenerate with the ignored `generate_magics` test; `stored_magics_are_valid` re-checks them.
pub const ROOK_MAGICS: [u64; 64] = [
    0x9080001184204004,
    0x00c01008a0004000,
    0x0500081100c12000,
    0x4700090084203000,
    0x9200060068210410,
    0x0080020014000980,
    0x00801a0001005080,
    0x0900018021450002,
    0x3000800220400480,
    0x2000400042201000,
    0x2201001020010940,
    0x8000801000800800,
    0x09010008020c1100,
    0x0602001012000884,
    0x0441002402000100,
    0x0140800244802100,
    0x9220608000c00090,
    0x0090054000c82000,
    0x0001010010406001,
    0x4801848018003000,
    0x0404050010080100,
    0x0001010006040008,
    0x0900040048900621,
    0x000102000a81004c,
    0x1c0440008004208c,
    0x1020100340042040,
    0x8d02002200348040,
    0x0000080080100080,
    0x1084240080080280,
    0x14890c0080020080,
    0x0081284400223001,
    0x0100884600108114,
    0x0002400020800880,
    0x0002814000802000,
    0x0020806000801004,
    0x800a002112004008,
    0x02c2800400800803,
    0x0040800400800200,
    0x08408a2804000110,
    0x040280c582000104,
    0x0080044460044004,
    0x6200a01000404001,
    0x0101002000450010,
    0x408a00c008120020,
    0x0201002800110004,
    0x2002008084008002,
    0x0813000600110004,
    0x0801000040810002,
    0xa001014024800500,
    0x02c0a08240090100,
    0x0c0a20001106c100,
    0x00a8000880100480,
    0x200908020c008080,
    0x020200504804c200,
    0x01010042000c1100,
    0x000580010000d080,
    0x0400209201048042,
    0x4a01015080400721,
    0x0702091020010045,
    0x0888100100082045,
    0x0013001004080023,
    0xc022000425081002,
    0xc201000200008401,
    0xc0000510e4440082,
];
/// Bishop magic for every square, found the same way (the search continues after the rook magics).
pub const BISHOP_MAGICS: [u64; 64] = [
    0x0088a00c04420420,
    0x0002840902021000,
    0x0008008102002000,
    0x0114124200105200,
    0x00051040400910c0,
    0x0042051008231410,
    0x00184c0444c01000,
    0x0000420050080400,
    0x0000102018430442,
    0x1080200444104440,
    0x0003280805002000,
    0x000108060b480010,
    0x0608011040800024,
    0x00020d1016500040,
    0x100904040c040601,
    0x0c04410900900400,
    0x4108000c08081842,
    0x4402001010021088,
    0x101000080081a228,
    0x0008094882044040,
    0x0084000081a00050,
    0x0001002210008401,
    0x0001002044022000,
    0x0600271100821000,
    0x4108200040840104,
    0x1004a00042080b00,
    0x80910100100408a2,
    0x220500c00c004200,
    0x08410040a4054000,
    0x0848060110c10088,
    0x10a4008704088c00,
    0x0423120001098980,
    0x0041041002202000,
    0x0284100c11488104,
    0x2012034040240100,
    0xa020120080080080,
    0x8802008400020160,
    0x0a05014200010500,
    0x0202020050040444,
    0x4304108028328410,
    0x0204090c40101001,
    0x0182008420004404,
    0x0000082088013000,
    0x7000204200806800,
    0x2033400811400202,
    0x0001100102000110,
    0x0345412602002400,
    0x0031240400810048,
    0x014300d004200480,
    0x0202048601108000,
    0x0018305200901080,
    0x0880058105882002,
    0x0008045082020000,
    0x1004303010012000,
    0x204002821c01000a,
    0x0804108400408144,
    0x0001008210030400,
    0x00001a0a12120611,
    0x0081008042080c00,
    0x1002002044840400,
    0x2820040010020202,
    0x2008024052041101,
    0x8000206002888100,
    0x0028020400440100,
];

/// The squares whose occupancy can change a rook's attacks from `sq`:
/// its rays, without `sq` and without the last square of each ray.
/// The file part ends on ranks 1 and 8, the rank part on files a and h, so each part drops only its own ends.
pub fn rook_mask(sq: u8) -> Bitboard {
    let file_part = file_mask(sq % 8) & !(RANK_1 | RANK_8);
    let rank_part = rank_mask(sq / 8) & !(FILE_A | FILE_H);
    let mut mask = file_part | rank_part;
    mask.clear(sq);
    mask
}

/// The squares whose occupancy can change a bishop's attacks from `sq`:
/// its diagonals, without `sq` and without the last square of each ray.
pub fn bishop_mask(sq: u8) -> Bitboard {
    bishop_attacks_ray(sq, Bitboard::EMPTY) & INNER_MASK
}

/// Every blocker pattern inside `mask`: all 2^n subsets, including the empty one and `mask` itself.
/// Doubling: start with only the empty set; for each mask square, copy every subset found so far
/// with that square added. After n squares there are 2^n subsets.
pub fn subsets(mask: Bitboard) -> Vec<Bitboard> {
    let mut result = Vec::with_capacity(1 << mask.count());
    result.push(Bitboard::EMPTY);
    for sq in mask {
        // `len` is read once, so the copies pushed below are not visited again in this round
        let len = result.len();
        for i in 0..len {
            let mut with_sq = result[i];
            with_sq.set(sq);
            result.push(with_sq);
        }
    }
    result
}

/// Squeezes a blocker `pattern` into a table index in `0..2^bits` using `magic`.
/// Uses wrapping multiplication: the overflow is part of the trick.
pub fn magic_index(pattern: Bitboard, magic: u64, bits: u32) -> usize {
    (pattern.bits.wrapping_mul(magic) >> (64 - bits)) as usize
}

/// `true` if no two blocker patterns of `slider` on `sq` share an index while having different attacks.
/// Fills a scratch table (`None` = empty slot) with every pattern's attacks; a slot that already
/// holds a different answer means the candidate is rejected.
pub fn is_magic(slider: Slider, sq: u8, magic: u64) -> bool {
    let mask = match slider {
        Slider::Rook => rook_mask(sq),
        Slider::Bishop => bishop_mask(sq),
    };
    let bits = mask.count();
    let mut table: Vec<Option<Bitboard>> = vec![None; 1 << bits];
    for pattern in subsets(mask) {
        let attacks = match slider {
            Slider::Rook => rook_attacks_ray(sq, pattern),
            Slider::Bishop => bishop_attacks_ray(sq, pattern),
        };
        let i = magic_index(pattern, magic, bits);
        match table[i] {
            None => table[i] = Some(attacks),
            Some(stored) if stored == attacks => {} // harmless collision: same answer
            Some(_) => return false,
        }
    }
    true
}

/// Guesses candidates from `rng` until one passes `is_magic`, and returns it.
pub fn find_magic(slider: Slider, sq: u8, rng: &mut Rng) -> u64 {
    loop {
        let magic = rng.next_sparse();
        if is_magic(slider, sq, magic) {
            return magic;
        }
    }
}

/// Small deterministic pseudo-random generator: the same seed always gives the same sequence,
/// so the same magics are found on every run.
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Starts a generator from `seed`, which must not be 0.
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// The next pseudo-random 64-bit number.
    pub fn next_rand(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// xorshift64*: the next xorshift number multiplied by a constant. The multiply breaks the
    /// pure-XOR (linear) structure of plain xorshift, so no fixed set of outputs XORs to zero.
    /// Used for Zobrist keys, where such XOR relations would cause systematic hash collisions.
    pub fn next_rand_star(&mut self) -> u64 {
        /// The xorshift64* output multiplier. Not derived: taken from L'Ecuyer's 1999 tables of 64-bit
        /// multipliers with good spectral-test scores (consecutive outputs spread evenly in 2D, 3D, ...),
        /// and used by Vigna's xorshift64* (and by Stockfish for its Zobrist keys). It is odd, so the
        /// multiply is invertible mod 2^64 and no two inputs give the same output.
        const XORSHIFT_STAR_MULTIPLIER: u64 = 0x2545_F491_4F6C_DD1D;
        self.next_rand().wrapping_mul(XORSHIFT_STAR_MULTIPLIER)
    }

    /// A random number with few set bits (about 8 of 64): a better magic candidate.
    pub fn next_sparse(&mut self) -> u64 {
        self.next_rand() & self.next_rand() & self.next_rand()
    }
}

/// Everything one square needs for a magic lookup: which squares matter, the multiplier,
/// how many index bits, and where this square's slots start in the shared `attacks` vector.
#[derive(Clone, Copy, Debug)]
pub struct MagicEntry {
    pub mask: Bitboard,
    pub magic: u64,
    pub bits: u32,
    pub offset: usize,
}

/// All magic attack tables for one slider, in one flat vector (layout A): square `sq` owns the
/// slots `offset .. offset + 2^bits` of `attacks`, one per blocker pattern.
pub struct MagicTable {
    entries: [MagicEntry; 64],
    attacks: Vec<Bitboard>,
}

impl MagicTable {
    /// The attacks of this slider on `sq` given `occupied`: mask, multiply, shift, one read.
    pub fn lookup(&self, sq: u8, occupied: Bitboard) -> Bitboard {
        let entry = self.entries[sq as usize];
        let index = magic_index(occupied & entry.mask, entry.magic, entry.bits);
        self.attacks[entry.offset + index]
    }
}

/// Builds the table for `slider` from the stored magics: for every square and every blocker
/// pattern, stores `*_attacks_ray` at the pattern's slot.
fn build_table(slider: Slider) -> MagicTable {
    let mut offset = 0;
    // 64 placeholder entries, each overwritten in the loop
    let mut entries = [MagicEntry {
        mask: Bitboard::EMPTY,
        magic: 0,
        bits: 0,
        offset: 0,
    }; 64];
    let size = match slider {
        Slider::Rook => ROOK_TABLE_SIZE,
        Slider::Bishop => BISHOP_TABLE_SIZE,
    };
    let mut attacks = vec![Bitboard::EMPTY; size];
    for sq in 0..64 {
        let mask = match slider {
            Slider::Rook => rook_mask(sq),
            Slider::Bishop => bishop_mask(sq),
        };
        let magic = match slider {
            Slider::Rook => ROOK_MAGICS[sq as usize],
            Slider::Bishop => BISHOP_MAGICS[sq as usize],
        };
        let bits = mask.count();
        // the entry belongs to the square: built once, with all its fields at once
        entries[sq as usize] = MagicEntry {
            mask,
            magic,
            bits,
            offset,
        };
        for pattern in subsets(mask) {
            let answer = match slider {
                Slider::Rook => rook_attacks_ray(sq, pattern),
                Slider::Bishop => bishop_attacks_ray(sq, pattern),
            };
            attacks[offset + magic_index(pattern, magic, bits)] = answer;
        }
        // the next square's slots start after this square's 2^bits slots
        offset += 2usize.pow(bits);
    }
    MagicTable { entries, attacks }
}

/// Rook tables, built on first use.
static ROOK_TABLE: LazyLock<MagicTable> = LazyLock::new(|| build_table(Slider::Rook));
/// Bishop tables, built on first use.
static BISHOP_TABLE: LazyLock<MagicTable> = LazyLock::new(|| build_table(Slider::Bishop));

/// Rook attacks from `sq` given `occupied`, by magic lookup in `ROOK_TABLE`.
/// Move generation calls `attacks::rook_attacks`, which picks this (or PEXT, later).
pub fn rook_attacks_magic(sq: u8, occupied: Bitboard) -> Bitboard {
    ROOK_TABLE.lookup(sq, occupied)
}

/// Bishop attacks from `sq` given `occupied`, by magic lookup in `BISHOP_TABLE`.
/// Move generation calls `attacks::bishop_attacks`, which picks this (or PEXT, later).
pub fn bishop_attacks_magic(sq: u8, occupied: Bitboard) -> Bitboard {
    BISHOP_TABLE.lookup(sq, occupied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attack_sliders::{bishop_attacks_ray, rook_attacks_ray};
    use crate::attacks::king_attacks;
    use crate::masks::{FILE_A, FILE_H, RANK_1, RANK_8};

    /// Builds a bitboard from a list of squares.
    fn squares(list: &[u8]) -> Bitboard {
        let mut bb = Bitboard::EMPTY;
        for &sq in list {
            bb.set(sq);
        }
        bb
    }

    const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

    // masks
    #[test]
    fn rook_mask_d4() {
        // d2 d3 d5 d6 d7, b4 c4 e4 f4 g4: no d1 d8 a4 h4 (ray ends) and no d4
        assert_eq!(
            rook_mask(27),
            squares(&[11, 19, 35, 43, 51, 25, 26, 28, 29, 30])
        );
    }
    #[test]
    fn rook_mask_corners() {
        // a1: a2..a7 and b1..g1
        assert_eq!(
            rook_mask(0),
            squares(&[8, 16, 24, 32, 40, 48, 1, 2, 3, 4, 5, 6])
        );
        // h8: h2..h7 and b8..g8
        assert_eq!(
            rook_mask(63),
            squares(&[15, 23, 31, 39, 47, 55, 57, 58, 59, 60, 61, 62])
        );
    }
    #[test]
    fn rook_mask_bit_counts() {
        // corners 12, other edge squares 11, inner squares 10
        for sq in 0..64 {
            let (file, rank) = (sq % 8, sq / 8);
            let on_file_edge = file == 0 || file == 7;
            let on_rank_edge = rank == 0 || rank == 7;
            let expected = match (on_file_edge, on_rank_edge) {
                (true, true) => 12,
                (true, false) | (false, true) => 11,
                (false, false) => 10,
            };
            assert_eq!(rook_mask(sq).count(), expected, "square {sq}");
        }
    }
    #[test]
    fn bishop_mask_d4_and_a1() {
        // d4: e5 f6 g7, c5 b6, e3 f2, c3 b2 (h8 a7 g1 a1 are ray ends)
        assert_eq!(
            bishop_mask(27),
            squares(&[36, 45, 54, 34, 41, 20, 13, 18, 9])
        );
        // a1: b2 .. g7
        assert_eq!(bishop_mask(0), squares(&[9, 18, 27, 36, 45, 54]));
    }
    #[test]
    fn bishop_mask_never_touches_the_board_edge() {
        let edges = FILE_A | FILE_H | RANK_1 | RANK_8;
        for sq in 0..64 {
            assert!((bishop_mask(sq) & edges).is_empty(), "square {sq}");
        }
    }
    #[test]
    fn masks_exclude_own_square_and_stay_inside_the_rays() {
        for sq in 0..64 {
            let rook = rook_mask(sq);
            let bishop = bishop_mask(sq);
            assert!(!rook.contains(sq) && !bishop.contains(sq), "square {sq}");
            // every mask square is attacked on an empty board
            let rook_rays = rook_attacks_ray(sq, Bitboard::EMPTY);
            let bishop_rays = bishop_attacks_ray(sq, Bitboard::EMPTY);
            assert_eq!(rook & rook_rays, rook, "rook {sq}");
            assert_eq!(bishop & bishop_rays, bishop, "bishop {sq}");
        }
    }
    #[test]
    fn total_table_sizes() {
        // sum of 2^bits over all squares: the standard magic table sizes
        let mut rook = 0;
        let mut bishop = 0;
        for sq in 0..64 {
            rook += 1 << rook_mask(sq).count();
            bishop += 1 << bishop_mask(sq).count();
        }
        assert_eq!(rook, ROOK_TABLE_SIZE);
        assert_eq!(bishop, BISHOP_TABLE_SIZE);
    }
    #[test]
    fn mask_lemma_squares_outside_the_mask_never_matter() {
        // the fact that makes magic correct: ray(occ) == ray(occ & mask) for any board
        let mut rng = Rng::new(SEED);
        for _ in 0..200 {
            let occupied = Bitboard {
                bits: rng.next_rand() & rng.next_rand(), // about 1/4 of the squares
            };
            for sq in 0..64 {
                assert_eq!(
                    rook_attacks_ray(sq, occupied),
                    rook_attacks_ray(sq, occupied & rook_mask(sq)),
                    "rook {sq}"
                );
                assert_eq!(
                    bishop_attacks_ray(sq, occupied),
                    bishop_attacks_ray(sq, occupied & bishop_mask(sq)),
                    "bishop {sq}"
                );
            }
        }
    }

    // subsets
    /// Checks that `subs` is exactly every subset of `mask`: 2^n of them, all different, all inside `mask`.
    fn assert_all_subsets(mask: Bitboard, subs: &[Bitboard]) {
        assert_eq!(subs.len(), 1 << mask.count());
        let mut bits: Vec<u64> = subs.iter().map(|s| s.bits).collect();
        bits.sort();
        bits.dedup();
        assert_eq!(bits.len(), subs.len(), "duplicates");
        for &s in subs {
            assert_eq!(s & mask, s, "{:#x} is not inside the mask", s.bits);
        }
        assert!(subs.contains(&Bitboard::EMPTY));
        assert!(subs.contains(&mask));
    }
    #[test]
    fn subsets_of_empty_mask_is_only_empty() {
        assert_eq!(subsets(Bitboard::EMPTY), vec![Bitboard::EMPTY]);
    }
    #[test]
    fn subsets_of_two_bits() {
        // the warm-up exercise: positions 1 and 4
        let mask = squares(&[1, 4]);
        let subs = subsets(mask);
        assert_all_subsets(mask, &subs);
        for expected in [squares(&[]), squares(&[1]), squares(&[4]), squares(&[1, 4])] {
            assert!(subs.contains(&expected), "{:#x}", expected.bits);
        }
    }
    #[test]
    fn subsets_of_rook_masks() {
        assert_all_subsets(rook_mask(27), &subsets(rook_mask(27))); // d4: 1024
        assert_all_subsets(rook_mask(0), &subsets(rook_mask(0))); // a1: 4096
    }

    // rng
    #[test]
    fn rng_same_seed_same_sequence() {
        let mut a = Rng::new(SEED);
        let mut b = Rng::new(SEED);
        for _ in 0..100 {
            assert_eq!(a.next_rand(), b.next_rand());
        }
    }
    #[test]
    fn rng_different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let same = (0..100).filter(|_| a.next_rand() == b.next_rand()).count();
        assert!(same < 100);
    }
    #[test]
    fn rng_never_returns_zero() {
        let mut rng = Rng::new(SEED);
        for _ in 0..10_000 {
            assert_ne!(rng.next_rand(), 0);
        }
    }
    #[test]
    fn rng_sparse_has_few_bits() {
        // three ANDs keep each bit with probability 1/8: about 8 of 64 bits on average
        let mut rng = Rng::new(SEED);
        let total: u32 = (0..1000).map(|_| rng.next_sparse().count_ones()).sum();
        let average = total as f64 / 1000.0;
        assert!((6.0..10.0).contains(&average), "average {average}");
    }
    #[test]
    fn rng_star_same_seed_same_sequence() {
        let mut a = Rng::new(SEED);
        let mut b = Rng::new(SEED);
        for _ in 0..100 {
            assert_eq!(a.next_rand_star(), b.next_rand_star());
        }
    }
    #[test]
    fn rng_star_is_xorshift_times_constant() {
        // pins the algorithm: same state steps as next_rand, output multiplied (wrapping).
        // The literal is repeated on purpose (not XORSHIFT_STAR_MULTIPLIER): a typo in the
        // constant would still pass if the test used the constant itself.
        let mut plain = Rng::new(SEED);
        let mut star = Rng::new(SEED);
        for _ in 0..100 {
            let expected = plain.next_rand().wrapping_mul(0x2545_F491_4F6C_DD1D);
            assert_eq!(star.next_rand_star(), expected);
        }
    }

    // magic_index
    #[test]
    fn magic_index_toy_up_ray() {
        // rook d4 looking up only: mask d5 d6 d7 (bits 35 43 51), magic 2^26 + 2^19 + 2^12
        // moves them to bits 61 62 63, so the index is the pattern read as (d7 d6 d5)
        let magic = (1 << 26) | (1 << 19) | (1 << 12);
        for pattern in subsets(squares(&[35, 43, 51])) {
            let expected = pattern.contains(35) as usize
                + 2 * pattern.contains(43) as usize
                + 4 * pattern.contains(51) as usize;
            assert_eq!(
                magic_index(pattern, magic, 3),
                expected,
                "{:#x}",
                pattern.bits
            );
        }
    }
    #[test]
    fn magic_index_real_d4_example() {
        // the magic found for a rook on d4 in the demo run
        let magic = 0x0800_1022_0042_0008;
        assert_eq!(magic_index(Bitboard::EMPTY, magic, 10), 0);
        assert_eq!(magic_index(squares(&[51]), magic, 10), 1); // d7
        assert_eq!(magic_index(squares(&[35]), magic, 10), 8); // d5
        assert_eq!(magic_index(squares(&[43]), magic, 10), 64); // d6
        assert_eq!(magic_index(squares(&[30, 51]), magic, 10), 513); // g4 d7
    }

    // is_magic
    #[test]
    fn is_magic_accepts_the_known_d4_magic() {
        assert!(is_magic(Slider::Rook, 27, 0x0800_1022_0042_0008));
    }
    #[test]
    fn is_magic_rejects_bad_candidates() {
        // 0: every pattern lands on index 0
        assert!(!is_magic(Slider::Rook, 27, 0));
        assert!(!is_magic(Slider::Bishop, 27, 0));
        // 1: no multiplication at all, the mask bits (at most bit 51) never reach the top 10
        assert!(!is_magic(Slider::Rook, 27, 1));
    }

    // find_magic
    /// Searches all 128 magics from `SEED` and prints them as the `ROOK_MAGICS` / `BISHOP_MAGICS`
    /// constants. Slow, so it only runs on demand:
    /// `cargo test --release -p board generate_magics -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn generate_magics() {
        let mut rng = Rng::new(SEED);
        for (slider, name) in [(Slider::Rook, "ROOK"), (Slider::Bishop, "BISHOP")] {
            println!("pub const {name}_MAGICS: [u64; 64] = [");
            for sq in 0..64 {
                let magic = find_magic(slider, sq, &mut rng);
                assert!(is_magic(slider, sq, magic), "{slider:?} {sq}");
                println!("    {magic:#018x},");
            }
            println!("];");
        }
    }
    #[test]
    fn stored_magics_are_valid() {
        for sq in 0..64 {
            assert!(
                is_magic(Slider::Rook, sq, ROOK_MAGICS[sq as usize]),
                "rook {sq}"
            );
            assert!(
                is_magic(Slider::Bishop, sq, BISHOP_MAGICS[sq as usize]),
                "bishop {sq}"
            );
        }
    }
    #[test]
    fn find_magic_is_deterministic() {
        for sq in [0, 27, 63] {
            let a = find_magic(Slider::Rook, sq, &mut Rng::new(SEED));
            let b = find_magic(Slider::Rook, sq, &mut Rng::new(SEED));
            assert_eq!(a, b, "square {sq}");
        }
    }

    // magic attack tables
    /// Every square occupied.
    const FULL: Bitboard = Bitboard { bits: u64::MAX };

    /// A few fixed boards used by several tests.
    const BOARDS: [Bitboard; 5] = [
        Bitboard::EMPTY,
        FULL,
        Bitboard {
            bits: 0xFFFF_0000_0000_FFFF,
        }, // start position
        Bitboard {
            bits: 0x0000_1824_4200_0000,
        }, // a few centre pieces
        Bitboard {
            bits: 0x55AA_55AA_55AA_55AA,
        }, // checkerboard
    ];

    #[test]
    fn every_pattern_matches_ray_walking() {
        // the complete proof: all 64 squares x all 2^bits patterns, 107,648 checks
        for sq in 0..64 {
            for pattern in subsets(rook_mask(sq)) {
                assert_eq!(
                    rook_attacks_magic(sq, pattern),
                    rook_attacks_ray(sq, pattern),
                    "rook {sq} {:#x}",
                    pattern.bits
                );
            }
            for pattern in subsets(bishop_mask(sq)) {
                assert_eq!(
                    bishop_attacks_magic(sq, pattern),
                    bishop_attacks_ray(sq, pattern),
                    "bishop {sq} {:#x}",
                    pattern.bits
                );
            }
        }
    }
    #[test]
    fn table_layout() {
        for (table, magics, total) in [
            (&*ROOK_TABLE, &ROOK_MAGICS, ROOK_TABLE_SIZE),
            (&*BISHOP_TABLE, &BISHOP_MAGICS, BISHOP_TABLE_SIZE),
        ] {
            assert_eq!(table.attacks.len(), total);
            assert_eq!(table.entries[0].offset, 0);
            for (sq, (entry, &magic)) in table.entries.iter().zip(magics).enumerate() {
                assert_eq!(entry.magic, magic, "square {sq}");
                assert_eq!(entry.bits, entry.mask.count(), "square {sq}");
            }
            // each square's slots start right after the previous square's
            for (sq, pair) in table.entries.windows(2).enumerate() {
                assert_eq!(
                    pair[1].offset,
                    pair[0].offset + (1 << pair[0].bits),
                    "square {sq}"
                );
            }
        }
        for sq in 0..64 {
            assert_eq!(ROOK_TABLE.entries[sq as usize].mask, rook_mask(sq));
            assert_eq!(BISHOP_TABLE.entries[sq as usize].mask, bishop_mask(sq));
        }
    }
    #[test]
    fn pieces_outside_the_mask_are_ignored() {
        // "board 2" from the demo: many pieces, but the rook on d4 only sees g4 and d7
        let board2 = squares(&[4, 59, 31, 24, 51, 30, 0, 63, 10, 45, 44]);
        assert_eq!(
            rook_attacks_magic(27, board2),
            rook_attacks_magic(27, squares(&[30, 51]))
        );
        assert_eq!(rook_attacks_magic(27, board2), rook_attacks_ray(27, board2));
        // bishop on d4: pieces on straight lines (d5 e4) and on ray ends (h8 a1 a7 g1) change nothing
        let straight_and_ends = squares(&[35, 28, 63, 0, 48, 6]);
        assert_eq!(
            bishop_attacks_magic(27, straight_and_ends),
            bishop_attacks_magic(27, Bitboard::EMPTY)
        );
    }
    #[test]
    fn demo_example_g4_d7() {
        // rook d4 blocked by g4 and d7: d1 d2 d3, a4 b4 c4, e4 f4 g4, d5 d6 d7
        let expected = squares(&[3, 11, 19, 24, 25, 26, 28, 29, 30, 35, 43, 51]);
        assert_eq!(rook_attacks_magic(27, squares(&[30, 51])), expected);
    }
    #[test]
    fn own_square_is_ignored() {
        for board in BOARDS {
            for sq in 0..64 {
                let with = board | squares(&[sq]);
                let mut without = board;
                without.clear(sq);
                assert_eq!(
                    rook_attacks_magic(sq, with),
                    rook_attacks_magic(sq, without),
                    "rook {sq}"
                );
                assert_eq!(
                    bishop_attacks_magic(sq, with),
                    bishop_attacks_magic(sq, without),
                    "bishop {sq}"
                );
            }
        }
    }
    #[test]
    fn empty_board_totals() {
        let mut rook = 0;
        let mut bishop = 0;
        for sq in 0..64 {
            rook += rook_attacks_magic(sq, Bitboard::EMPTY).count();
            bishop += bishop_attacks_magic(sq, Bitboard::EMPTY).count();
        }
        assert_eq!(rook, 896);
        assert_eq!(bishop, 560);
    }
    #[test]
    fn full_board_rook_plus_bishop_is_king() {
        for sq in 0..64 {
            assert_eq!(
                rook_attacks_magic(sq, FULL) | bishop_attacks_magic(sq, FULL),
                king_attacks(sq),
                "square {sq}"
            );
        }
    }
    #[test]
    fn corners_match_ray_walking() {
        for sq in [0, 7, 56, 63] {
            for board in [Bitboard::EMPTY, FULL] {
                assert_eq!(
                    rook_attacks_magic(sq, board),
                    rook_attacks_ray(sq, board),
                    "rook {sq}"
                );
                assert_eq!(
                    bishop_attacks_magic(sq, board),
                    bishop_attacks_ray(sq, board),
                    "bishop {sq}"
                );
            }
        }
    }
}
