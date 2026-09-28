//! Square names: converts between a square index (a1 = 0 … h8 = 63) and its text name ("e3").
//! Used only at the text boundaries (FEN, UCI); the engine itself works with indices.

/// Parses a lowercase square name like `"e3"` into its index (`rank * 8 + file`).
/// Returns `None` unless the input is exactly a file `a`–`h` followed by a rank `1`–`8`.
pub fn square_from_name(name: &str) -> Option<u8> {
    let mut chars = name.chars();
    let file = chars.next()?;
    let rank = chars.next()?;
    if chars.next().is_some() || !('a'..='h').contains(&file) || !('1'..='8').contains(&rank) {
        return None;
    }
    Some((rank as u8 - b'1') * 8 + (file as u8 - b'a'))
}

/// Returns the name of a square index, e.g. `20` → `"e3"`.
pub fn square_name(sq: u8) -> String {
    let file = (b'a' + sq % 8) as char;
    let rank = (b'1' + sq / 8) as char;
    format!("{file}{rank}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1. corners and a middle square
    #[test]
    fn known_squares_parse() {
        assert_eq!(square_from_name("a1"), Some(0));
        assert_eq!(square_from_name("h1"), Some(7));
        assert_eq!(square_from_name("a8"), Some(56));
        assert_eq!(square_from_name("h8"), Some(63));
        assert_eq!(square_from_name("e3"), Some(20));
    }

    // 2. every index survives index -> name -> index
    #[test]
    fn every_square_round_trips() {
        for sq in 0..64 {
            assert_eq!(square_from_name(&square_name(sq)), Some(sq), "square {sq}");
        }
    }

    // 3. off-board, wrong case, wrong length
    #[test]
    fn invalid_names_are_rejected() {
        for name in ["e9", "i1", "e0", "E3", "e", "e33", "", "-"] {
            assert_eq!(square_from_name(name), None, "{name:?}");
        }
    }
}
