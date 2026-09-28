use crate::color::Color;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
} // in this order
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Piece {
    pub color: Color,
    pub kind: PieceKind,
}

impl Piece {
    pub fn from_fen_char(c: char) -> Option<Piece> {
        match c {
            'P' => Some(Piece {
                color: Color::White,
                kind: PieceKind::Pawn,
            }),
            'p' => Some(Piece {
                color: Color::Black,
                kind: PieceKind::Pawn,
            }),
            'K' => Some(Piece {
                color: Color::White,
                kind: PieceKind::King,
            }),
            'k' => Some(Piece {
                color: Color::Black,
                kind: PieceKind::King,
            }),
            'N' => Some(Piece {
                color: Color::White,
                kind: PieceKind::Knight,
            }),
            'n' => Some(Piece {
                color: Color::Black,
                kind: PieceKind::Knight,
            }),
            'B' => Some(Piece {
                color: Color::White,
                kind: PieceKind::Bishop,
            }),
            'b' => Some(Piece {
                color: Color::Black,
                kind: PieceKind::Bishop,
            }),
            'R' => Some(Piece {
                color: Color::White,
                kind: PieceKind::Rook,
            }),
            'r' => Some(Piece {
                color: Color::Black,
                kind: PieceKind::Rook,
            }),
            'Q' => Some(Piece {
                color: Color::White,
                kind: PieceKind::Queen,
            }),
            'q' => Some(Piece {
                color: Color::Black,
                kind: PieceKind::Queen,
            }),
            _ => None,
        }
    }
    pub fn to_fen_char(self) -> char {
        let letter = match self.kind {
            PieceKind::Pawn => 'p',
            PieceKind::Knight => 'n',
            PieceKind::Bishop => 'b',
            PieceKind::Rook => 'r',
            PieceKind::Queen => 'q',
            PieceKind::King => 'k',
        };
        match self.color {
            Color::White => letter.to_ascii_uppercase(),
            Color::Black => letter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1. letter case decides the color
    #[test]
    fn from_fen_char_reads_kind_and_color() {
        assert_eq!(
            Piece::from_fen_char('P'),
            Some(Piece {
                color: Color::White,
                kind: PieceKind::Pawn
            })
        );
        assert_eq!(
            Piece::from_fen_char('k'),
            Some(Piece {
                color: Color::Black,
                kind: PieceKind::King
            })
        );
    }

    // 2. every piece letter survives char -> Piece -> char
    #[test]
    fn every_piece_letter_round_trips() {
        for c in "PNBRQKpnbrqk".chars() {
            let piece = Piece::from_fen_char(c).expect("valid piece letter");
            assert_eq!(piece.to_fen_char(), c);
        }
    }

    // 3. anything that is not a piece letter is rejected
    #[test]
    fn non_piece_chars_are_rejected() {
        for c in ['x', '1', ' ', 'E'] {
            assert_eq!(Piece::from_fen_char(c), None, "{c:?}");
        }
    }

    // 4. `kind as usize` indexes pieces[color][kind], so the order is fixed
    #[test]
    fn kind_order_matches_table_index() {
        assert_eq!(PieceKind::Pawn as usize, 0);
        assert_eq!(PieceKind::Knight as usize, 1);
        assert_eq!(PieceKind::Bishop as usize, 2);
        assert_eq!(PieceKind::Rook as usize, 3);
        assert_eq!(PieceKind::Queen as usize, 4);
        assert_eq!(PieceKind::King as usize, 5);
    }
}
