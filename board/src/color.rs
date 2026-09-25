//! The two sides of the game.

/// A side: White moves up the board (towards rank 8), Black moves down.
/// `color as usize` is 0 for White and 1 for Black, handy as a table index.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    White,
    Black,
}
impl Color {
    /// +1 for White (moves up), -1 for Black (moves down).
    pub fn forward(self) -> i8 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }
}
