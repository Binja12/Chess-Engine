//! `Rng`: a small seeded random number generator (xorshift64*), and `pick`, which picks `n`
//! different items of a list at random (the random engine's moves). The same seed always gives
//! the same numbers, so the random engine plays the same moves and a game can be replayed
//! exactly. Rust's standard library has no random numbers, and this is all the random engine
//! needs.

/// Used instead of a seed of 0, since xorshift started at 0 stays at 0 forever. Any fixed
/// non-zero number works; this one is 2^64 divided by the golden ratio.
const SEED_FOR_ZERO: u64 = 0x9E37_79B9_7F4A_7C15;

/// The xorshift64* output multiplier (Vigna's, the same as the board crate's `Rng`). It is odd,
/// so multiplying by it mixes the bits without losing any.
const MULTIPLIER: u64 = 0x2545_F491_4F6C_DD1D;

/// A deterministic pseudo-random number generator: the same seed always gives the same numbers.
pub(crate) struct Rng {
    /// The xorshift state, never 0.
    state: u64,
}

impl Rng {
    /// A generator started from `seed`. Every seed works, 0 too.
    pub(crate) fn new(seed: u64) -> Rng {
        let state = if seed == 0 { SEED_FOR_ZERO } else { seed };
        Rng { state }
    }

    /// The next pseudo-random 64-bit number: one xorshift step on the state, then a multiply that
    /// mixes its bits (xorshift64*).
    pub(crate) fn next_rand(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state.wrapping_mul(MULTIPLIER)
    }

    /// A pseudo-random index from 0 to `n - 1`, each about equally likely: for picking one of `n`
    /// items. Panics if `n` is 0, since there is no number below 0.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next_rand() % n as u64) as usize
    }

    /// `n` different items of `items`, picked at random and in random order: every item has the
    /// same chance to be picked, and to come first. All of them, shuffled, when there are at most
    /// `n`; none when `n` is 0 or `items` is empty.
    pub(crate) fn pick<T>(&mut self, mut items: Vec<T>, n: usize) -> Vec<T> {
        // a Fisher-Yates shuffle that stops after `count` swaps: each swap brings a random one of
        // the items not picked yet to the next place
        let count = n.min(items.len());
        for i in 0..count {
            let j = i + self.below(items.len() - i);
            items.swap(i, j);
        }
        items.truncate(count);
        items
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;
    use std::collections::HashSet;

    // 1. the same seed gives the same numbers
    #[test]
    fn the_same_seed_gives_the_same_numbers() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_rand(), b.next_rand());
        }
    }

    // 2. different seeds give different numbers
    #[test]
    fn different_seeds_give_different_numbers() {
        let numbers = |seed| {
            let mut rng = Rng::new(seed);
            (0..100).map(|_| rng.next_rand()).collect::<Vec<u64>>()
        };
        assert_ne!(numbers(1), numbers(2));
    }

    // 3. seed 0 works: 100 numbers in a row are all different (plain xorshift started at 0 would
    //    give 0 every time)
    #[test]
    fn seed_zero_works() {
        let mut rng = Rng::new(0);
        let numbers: HashSet<u64> = (0..100).map(|_| rng.next_rand()).collect();
        assert_eq!(numbers.len(), 100);
    }

    // 4. below(n) is always less than n and gives every value from 0 to n - 1: for 1 (a single
    //    legal move), 2, 20 (the start position) and 218 (the most legal moves a chess position
    //    can have)
    #[test]
    fn below_gives_every_value_from_0_to_n_minus_1() {
        let mut rng = Rng::new(4);
        for n in [1, 2, 20, 218] {
            let mut seen = vec![false; n];
            for _ in 0..100 * n {
                let value = rng.below(n);
                assert!(value < n, "below({n}) gave {value}");
                seen[value] = true;
            }
            let missing: Vec<usize> = (0..n).filter(|&value| !seen[value]).collect();
            assert!(missing.is_empty(), "below({n}) never gave {missing:?}");
        }
    }

    // 5. below(0) panics: there is no number below 0
    #[test]
    #[should_panic(expected = "divisor of zero")]
    fn below_zero_panics() {
        Rng::new(5).below(0);
    }

    // 6. pick(n) gives n different items of the list
    #[test]
    fn pick_gives_n_different_items() {
        let mut rng = Rng::new(6);
        for _ in 0..100 {
            let picked = rng.pick((0..20u32).collect(), 3);
            let different: HashSet<u32> = picked.iter().copied().collect();
            assert!(
                picked.len() == 3 && different.len() == 3 && picked.iter().all(|&item| item < 20),
                "{picked:?}"
            );
        }
    }

    // 7. pick gives every item once when n is at least their number, up to usize::MAX, and none
    //    when n is 0 or the list is empty
    #[test]
    fn pick_gives_every_item_once_when_n_is_large() {
        let mut rng = Rng::new(7);
        let all: Vec<u32> = (0..20).collect();
        for n in [20, 21, usize::MAX] {
            let mut picked = rng.pick(all.clone(), n);
            picked.sort();
            assert_eq!(picked, all, "n = {n}");
        }
        assert!(rng.pick(all, 0).is_empty(), "n = 0");
        assert!(rng.pick(Vec::<u32>::new(), 5).is_empty(), "an empty list");
    }

    // 8. every item comes first about equally often: in 10,000 picks of 3 out of 20, each item
    //    comes first between half and twice its fair share of 500
    #[test]
    fn every_item_comes_first_about_equally_often() {
        let mut rng = Rng::new(8);
        let mut firsts = [0usize; 20];
        for _ in 0..10_000 {
            firsts[rng.pick((0..20).collect(), 3)[0]] += 1;
        }
        for (item, &count) in firsts.iter().enumerate() {
            assert!(
                (250..=1000).contains(&count),
                "item {item} came first {count} times in 10,000 picks"
            );
        }
    }
}
