//! `Position`: the full state of a game at one moment, and FEN (the standard text form of it).
//!
//! Pieces are stored twice: as 12 bitboards (fast set questions: attacks, move generation)
//! and as a 64-square mailbox (fast "what is on this square?"). Both must always agree.

use crate::bitboard::Bitboard;
use crate::color::Color;
use crate::masks::{RANK_1, RANK_8};
use crate::piece::{Piece, PieceKind};
use crate::square::{square_from_name, square_name};
use crate::zobrist::{black_to_move_key, castling_key, en_passant_key, piece_key};

/// Castling right flag: White may still castle kingside (FEN `K`).
pub const WHITE_KINGSIDE: u8 = 1;
/// Castling right flag: White may still castle queenside (FEN `Q`).
pub const WHITE_QUEENSIDE: u8 = 2;
/// Castling right flag: Black may still castle kingside (FEN `k`).
pub const BLACK_KINGSIDE: u8 = 4;
/// Castling right flag: Black may still castle queenside (FEN `q`).
pub const BLACK_QUEENSIDE: u8 = 8;

/// FEN of the standard starting position.
pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// The full state of a game: where every piece is, whose turn it is, and the rule state
/// (castling rights, en passant square, move counters).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Position {
    /// `pieces[color][kind]`: the squares holding that piece.
    pieces: [[Bitboard; 6]; 2],
    /// `colors[color]`: every square holding a piece of that color.
    colors: [Bitboard; 2],
    /// `mailbox[sq]`: the piece on `sq`, or `None` when it is empty.
    mailbox: [Option<Piece>; 64],
    side_to_move: Color,
    /// Castling rights, an OR of the `WHITE_*` / `BLACK_*` flags.
    castling: u8,
    /// The square a pawn skipped with a double push on the last move, if any.
    en_passant: Option<u8>,
    /// Half-moves since the last capture or pawn move (50-move rule).
    halfmove_clock: u16,
    /// Starts at 1, increases after every Black move.
    fullmove_number: u16,
    /// Zobrist hash of everything except the move counters, kept up to date incrementally.
    hash: u64,
}

/// Why a FEN string could not be parsed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FenError {
    /// Not 4 or 6 space-separated fields.
    WrongFieldCount,
    /// The board field does not have 8 ranks.
    BadRankCount,
    /// A rank does not add up to exactly 8 squares, or has two digits in a row, or a 0 / 9.
    BadRankLength,
    /// A character in the board field that is not a piece letter or a digit.
    BadPiece(char),
    /// Side to move is not `w` or `b`.
    BadSideToMove,
    /// Castling is not `-` or a set of distinct `KQkq` letters.
    BadCastling,
    /// En passant is not `-` or a square on the right rank for the side to move.
    BadEnPassant,
    /// Halfmove clock or fullmove number is not a number.
    BadClock,
    /// A side does not have exactly one king.
    BadKingCount,
    /// A pawn stands on rank 1 or rank 8.
    PawnOnBackRank,
}

impl Position {
    /// No pieces, White to move, no rights, counters `0 1`. The starting point for building a
    /// position; private because a board without kings is not a valid position.
    const EMPTY: Position = Position {
        pieces: [[Bitboard::EMPTY; 6]; 2],
        colors: [Bitboard::EMPTY; 2],
        mailbox: [None; 64],
        side_to_move: Color::White,
        castling: 0,
        en_passant: None,
        halfmove_clock: 0,
        fullmove_number: 1,
        hash: 0,
    };

    /// Parses a FEN string. Accepts 6 fields, or 4 (counters then default to `0 1`).
    pub fn from_fen(fen: &str) -> Result<Position, FenError> {
        let fields: Vec<&str> = fen.split_whitespace().collect();
        if fields.len() != 4 && fields.len() != 6 {
            return Err(FenError::WrongFieldCount);
        }
        let mut pos = Position::EMPTY;
        pos.parse_board(fields[0])?;
        pos.side_to_move = parse_side(fields[1])?;
        pos.castling = parse_castling(fields[2])?;
        pos.en_passant = parse_en_passant(fields[3], pos.side_to_move)?;
        pos.hash ^= castling_key(pos.castling);
        if let Some(sq) = pos.en_passant {
            pos.hash ^= en_passant_key(sq % 8);
        }
        if pos.side_to_move == Color::Black {
            pos.hash ^= black_to_move_key();
        }
        if fields.len() == 6 {
            pos.halfmove_clock = parse_clock(fields[4])?;
            pos.fullmove_number = parse_clock(fields[5])?;
        }
        pos.check_kings_and_pawns()?;
        Ok(pos)
    }

    /// Writes the position as a 6-field FEN string, castling in `KQkq` order.
    pub fn to_fen(&self) -> String {
        let mut fen = String::new();
        for rank in (0..8).rev() {
            let mut empty = 0u8;
            for file in 0..8 {
                match self.mailbox[rank * 8 + file] {
                    Some(piece) => {
                        if empty != 0 {
                            fen.push((b'0' + empty) as char);
                            empty = 0;
                        }
                        fen.push(piece.to_fen_char());
                    }
                    None => empty += 1,
                }
            }
            if empty != 0 {
                fen.push((b'0' + empty) as char);
            }
            if rank > 0 {
                fen.push('/');
            }
        }
        fen.push(' ');
        fen.push(match self.side_to_move {
            Color::Black => 'b',
            Color::White => 'w',
        });
        fen.push(' ');
        if self.castling == 0 {
            fen.push('-');
        } else {
            if self.castling & WHITE_KINGSIDE != 0 {
                fen.push('K');
            }
            if self.castling & WHITE_QUEENSIDE != 0 {
                fen.push('Q');
            }
            if self.castling & BLACK_KINGSIDE != 0 {
                fen.push('k');
            }
            if self.castling & BLACK_QUEENSIDE != 0 {
                fen.push('q');
            }
        }
        fen.push(' ');
        match self.en_passant {
            Some(sq) => fen.push_str(&square_name(sq)),
            None => fen.push('-'),
        }
        fen.push_str(&format!(
            " {} {}",
            self.halfmove_clock, self.fullmove_number
        ));
        fen
    }

    /// The squares holding pieces of this color and kind.
    pub fn pieces(&self, color: Color, kind: PieceKind) -> Bitboard {
        self.pieces[color as usize][kind as usize]
    }

    /// Every square holding a piece of this color.
    pub fn color_pieces(&self, color: Color) -> Bitboard {
        self.colors[color as usize]
    }

    /// Every square holding any piece.
    pub fn occupied(&self) -> Bitboard {
        self.colors[0] | self.colors[1]
    }

    /// The piece on `sq`, or `None` when the square is empty.
    pub fn piece_at(&self, sq: u8) -> Option<Piece> {
        self.mailbox[sq as usize]
    }

    /// The side whose turn it is.
    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    /// Castling rights, an OR of the `WHITE_*` / `BLACK_*` flags.
    pub fn castling(&self) -> u8 {
        self.castling
    }

    /// The en passant target square, if the last move was a double pawn push.
    pub fn en_passant(&self) -> Option<u8> {
        self.en_passant
    }

    /// Half-moves since the last capture or pawn move.
    pub fn halfmove_clock(&self) -> u16 {
        self.halfmove_clock
    }

    /// The move number as people write it, starting at 1.
    pub fn fullmove_number(&self) -> u16 {
        self.fullmove_number
    }

    /// The Zobrist hash: equal positions (ignoring the move counters) have equal hashes.
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// Recomputes the hash from scratch. Slow; used by tests and debug checks to verify that
    /// the incrementally updated `hash()` is right.
    pub fn compute_hash(&self) -> u64 {
        let mut hash = 0;
        for sq in 0..64 {
            if let Some(piece) = self.mailbox[sq] {
                hash ^= piece_key(piece.color, piece.kind, sq as u8);
            }
        }
        hash ^= castling_key(self.castling);
        if let Some(sq) = self.en_passant {
            hash ^= en_passant_key(sq % 8);
        }
        if self.side_to_move == Color::Black {
            hash ^= black_to_move_key();
        }
        hash
    }
}

/// Lets a position be parsed with `"...".parse::<Position>()`, the same as `from_fen`.
impl std::str::FromStr for Position {
    type Err = FenError;

    fn from_str(fen: &str) -> Result<Position, FenError> {
        Position::from_fen(fen)
    }
}

/// A readable message for each error, e.g. for UCI `info string` output.
impl std::fmt::Display for FenError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            FenError::WrongFieldCount => write!(f, "FEN must have 4 or 6 space-separated fields"),
            FenError::BadRankCount => write!(f, "FEN board must have 8 ranks separated by '/'"),
            FenError::BadRankLength => write!(f, "a FEN rank does not add up to 8 squares"),
            FenError::BadPiece(c) => write!(f, "unknown piece letter '{c}' in FEN board"),
            FenError::BadSideToMove => write!(f, "FEN side to move must be 'w' or 'b'"),
            FenError::BadCastling => {
                write!(f, "FEN castling must be '-' or distinct letters from KQkq")
            }
            FenError::BadEnPassant => write!(
                f,
                "FEN en passant must be '-' or a rank 3/6 square matching the side to move"
            ),
            FenError::BadClock => write!(f, "FEN move counters must be non-negative numbers"),
            FenError::BadKingCount => write!(f, "each side must have exactly one king"),
            FenError::PawnOnBackRank => write!(f, "pawns cannot stand on rank 1 or 8"),
        }
    }
}

/// Marks `FenError` as a standard error type, so it works with `?` and `Box<dyn Error>`.
impl std::error::Error for FenError {}

// ---------- helpers ----------

impl Position {
    /// Puts `piece` on the empty square `sq`, updating the bitboards and the mailbox together.
    fn put_piece(&mut self, piece: Piece, sq: u8) {
        self.pieces[piece.color as usize][piece.kind as usize].set(sq);
        self.colors[piece.color as usize].set(sq);
        self.mailbox[sq as usize] = Some(piece);
        self.hash ^= piece_key(piece.color, piece.kind, sq);
    }

    /// Reads the FEN board field (rank 8 first, ranks split by `/`) and places the pieces.
    fn parse_board(&mut self, board: &str) -> Result<(), FenError> {
        let ranks: Vec<&str> = board.split('/').collect();
        if ranks.len() != 8 {
            return Err(FenError::BadRankCount);
        }
        for (i, rank_text) in ranks.iter().enumerate() {
            let rank = 7 - i as u8;
            let mut file = 0u8;
            let mut last_was_digit = false;
            for c in rank_text.chars() {
                if let Some(digit) = c.to_digit(10) {
                    // a run of empty squares: 1-8, never two digits in a row
                    if last_was_digit || digit == 0 || digit > 8 {
                        return Err(FenError::BadRankLength);
                    }
                    file += digit as u8;
                    last_was_digit = true;
                } else {
                    let piece = Piece::from_fen_char(c).ok_or(FenError::BadPiece(c))?;
                    if file >= 8 {
                        return Err(FenError::BadRankLength);
                    }
                    self.put_piece(piece, rank * 8 + file);
                    file += 1;
                    last_was_digit = false;
                }
                if file > 8 {
                    return Err(FenError::BadRankLength);
                }
            }
            if file != 8 {
                return Err(FenError::BadRankLength);
            }
        }
        Ok(())
    }

    /// Rejects boards that would break move generation: each side needs exactly one king,
    /// and no pawn may stand on rank 1 or 8.
    fn check_kings_and_pawns(&self) -> Result<(), FenError> {
        for color in [Color::White, Color::Black] {
            if self.pieces(color, PieceKind::King).count() != 1 {
                return Err(FenError::BadKingCount);
            }
        }
        let pawns =
            self.pieces(Color::White, PieceKind::Pawn) | self.pieces(Color::Black, PieceKind::Pawn);
        if !(pawns & (RANK_1 | RANK_8)).is_empty() {
            return Err(FenError::PawnOnBackRank);
        }
        Ok(())
    }
}

/// Reads the side-to-move field: `w` or `b`.
fn parse_side(text: &str) -> Result<Color, FenError> {
    match text {
        "w" => Ok(Color::White),
        "b" => Ok(Color::Black),
        _ => Err(FenError::BadSideToMove),
    }
}

/// Reads the castling field: `-`, or distinct letters from `KQkq`.
fn parse_castling(text: &str) -> Result<u8, FenError> {
    if text == "-" {
        return Ok(0);
    }
    let mut rights = 0;
    for c in text.chars() {
        let flag = match c {
            'K' => WHITE_KINGSIDE,
            'Q' => WHITE_QUEENSIDE,
            'k' => BLACK_KINGSIDE,
            'q' => BLACK_QUEENSIDE,
            _ => return Err(FenError::BadCastling),
        };
        if rights & flag != 0 {
            return Err(FenError::BadCastling);
        }
        rights |= flag;
    }
    Ok(rights)
}

/// Reads the en passant field: `-`, or a square on rank 6 (White to move) or rank 3
/// (Black to move), the square the opponent's pawn just skipped.
fn parse_en_passant(text: &str, side: Color) -> Result<Option<u8>, FenError> {
    if text == "-" {
        return Ok(None);
    }
    let sq = square_from_name(text).ok_or(FenError::BadEnPassant)?;
    let expected_rank = match side {
        Color::White => 5,
        Color::Black => 2,
    };
    if sq / 8 != expected_rank {
        return Err(FenError::BadEnPassant);
    }
    Ok(Some(sq))
}

/// Reads a move counter (halfmove clock or fullmove number).
fn parse_clock(text: &str) -> Result<u16, FenError> {
    text.parse::<u16>().map_err(|_| FenError::BadClock)
}
