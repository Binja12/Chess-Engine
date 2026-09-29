//! `Position`: the full state of a game at one moment, and FEN (the standard text form of it).
//!
//! Pieces are stored twice: as 12 bitboards (fast set questions: attacks, move generation)
//! and as a 64-square mailbox (fast "what is on this square?"). Both must always agree.

use crate::attacks::{bishop_attacks, king_attacks, knight_attacks, pawn_attacks, rook_attacks};
use crate::bitboard::Bitboard;
use crate::castling::{CASTLES, castle, rights_kept};
use crate::color::Color;
use crate::masks::{RANK_1, RANK_8};
use crate::moves::{DOUBLE_PAWN_PUSH, EN_PASSANT, KING_CASTLE, Move, QUEEN_CASTLE};
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
    /// The square a pawn skipped with a double push on the last move, kept only when an enemy
    /// pawn could capture onto it (otherwise the position is the same as without it).
    en_passant: Option<u8>,
    /// Half-moves since the last capture or pawn move (50-move rule).
    halfmove_clock: u16,
    /// Starts at 1, increases after every Black move.
    fullmove_number: u16,
    /// Zobrist hash of everything except the move counters, kept up to date incrementally.
    hash: u64,
}

/// What `make_move` destroys and `unmake_move` needs back: `make_move` returns it, the caller
/// keeps it (in search: a local variable of that depth) and hands it to `unmake_move`. It is not
/// `Clone`, so passing it to `unmake_move` moves it and each record can be used only once.
#[derive(Debug)]
pub struct Undo {
    /// The piece the move captured; for en passant, the pawn beside the to square.
    captured: Option<Piece>,
    /// Castling rights before the move.
    castling: u8,
    /// En passant square before the move.
    en_passant: Option<u8>,
    /// Halfmove clock before the move.
    halfmove_clock: u16,
    /// Hash before the move.
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
    /// A castling right whose king or rook is not on its home square.
    CastlingWithoutPieces,
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
    /// An en passant square that no pawn of the side to move could capture onto is dropped, the
    /// same rule `make_move` follows, so the same position always gets the same hash.
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
        if let Some(ep) = pos.en_passant {
            // the pawn that skipped `ep` belongs to the side that just moved
            pos.en_passant = pos.usable_en_passant(ep, opponent(pos.side_to_move));
        }
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
        pos.check_castling_rights()?;
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

    /// True if any piece of color `by` attacks `sq`. Looks outwards from `sq`: a knight on `sq`
    /// would hit `by`'s knights, a bishop on `sq` would hit `by`'s bishops and queens, and so on.
    pub fn is_attacked(&self, sq: u8, by: Color) -> bool {
        let occ = self.occupied();
        if (bishop_attacks(sq, occ)
            & (self.pieces(by, PieceKind::Bishop) | self.pieces(by, PieceKind::Queen)))
            != Bitboard::EMPTY
        {
            return true;
        }
        if (rook_attacks(sq, occ)
            & (self.pieces(by, PieceKind::Rook) | self.pieces(by, PieceKind::Queen)))
            != Bitboard::EMPTY
        {
            return true;
        }
        if (knight_attacks(sq) & (self.pieces(by, PieceKind::Knight))) != Bitboard::EMPTY {
            return true;
        }
        if (pawn_attacks(opponent(by), sq) & self.pieces(by, PieceKind::Pawn)) != Bitboard::EMPTY {
            return true;
        }
        if (king_attacks(sq) & self.pieces(by, PieceKind::King)) != Bitboard::EMPTY {
            return true;
        }
        false
    }

    /// Plays `mv`, a move `generate_moves` produced for this position, and returns what is
    /// needed to take it back. Moves the pieces (the rook too when castling, the captured pawn
    /// beside the to square for en passant, the new piece for a promotion) and updates the side
    /// to move, castling rights, en passant square (set only when an enemy pawn could capture
    /// onto it), move counters and hash. The move is trusted: whether it leaves the mover's king
    /// in check is not tested here.
    pub fn make_move(&mut self, mv: Move) -> Undo {
        let (from, to, flag) = (mv.from(), mv.to(), mv.flag());
        let us = self.side_to_move;
        let moved = self
            .piece_at(from)
            .expect("make_move: no piece on the from square");
        // everything below can change these, so keep them for unmake first
        let mut undo = Undo {
            captured: None,
            castling: self.castling,
            en_passant: self.en_passant,
            halfmove_clock: self.halfmove_clock,
            hash: self.hash,
        };

        // pieces: the captured one leaves, the moving one goes from `from` to `to`
        if mv.is_capture() {
            undo.captured = Some(self.remove_piece(captured_square(mv, us)));
        }
        self.remove_piece(from);
        let arriving = match mv.promotion() {
            Some(kind) => Piece { color: us, kind },
            None => moved,
        };
        self.put_piece(arriving, to);
        if flag == KING_CASTLE || flag == QUEEN_CASTLE {
            // the rook jumps over the king onto the square the king crossed
            let castle = castle(us, flag);
            self.move_piece(castle.rook_from, castle.king_crosses);
        }

        // state: castling rights, en passant square, counters, turn
        if self.castling != 0 {
            self.set_castling(self.castling & rights_kept(from) & rights_kept(to));
        }
        let en_passant = if flag == DOUBLE_PAWN_PUSH {
            // the skipped square is halfway between from and to
            self.usable_en_passant((from + to) / 2, us)
        } else {
            None
        };
        self.set_en_passant(en_passant);
        self.halfmove_clock = if moved.kind == PieceKind::Pawn || mv.is_capture() {
            0
        } else {
            self.halfmove_clock + 1
        };
        if us == Color::Black {
            self.fullmove_number += 1;
        }
        self.side_to_move = opponent(us);
        self.hash ^= black_to_move_key();
        undo
    }

    /// Takes back `mv`, which must be the last move made, with the `undo` that `make_move`
    /// returned for it. Afterwards the position is exactly as before `make_move`, hash included.
    pub fn unmake_move(&mut self, mv: Move, undo: Undo) {
        let (from, to, flag) = (mv.from(), mv.to(), mv.flag());
        // the side that made the move is to move again
        let us = opponent(self.side_to_move);
        self.side_to_move = us;
        if us == Color::Black {
            self.fullmove_number -= 1;
        }

        // pieces, in reverse: rook back, moved piece back (a promoted piece becomes a pawn
        // again), then the captured piece returns
        if flag == KING_CASTLE || flag == QUEEN_CASTLE {
            let castle = castle(us, flag);
            self.move_piece(castle.king_crosses, castle.rook_from);
        }
        let arrived = self.remove_piece(to);
        let moved = if mv.promotion().is_some() {
            Piece {
                color: us,
                kind: PieceKind::Pawn,
            }
        } else {
            arrived
        };
        self.put_piece(moved, from);
        if let Some(captured) = undo.captured {
            self.put_piece(captured, captured_square(mv, us));
        }

        // state: straight from the record (the piece helpers updated the hash on the way, but
        // copying the old one back is simpler than undoing each change)
        self.castling = undo.castling;
        self.en_passant = undo.en_passant;
        self.halfmove_clock = undo.halfmove_clock;
        self.hash = undo.hash;
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
            FenError::CastlingWithoutPieces => write!(
                f,
                "a castling right needs its king and rook on their home squares"
            ),
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

    /// Takes the piece off `sq` (there must be one) and returns it; the opposite of `put_piece`.
    fn remove_piece(&mut self, sq: u8) -> Piece {
        let piece = self.mailbox[sq as usize].expect("remove_piece: the square is empty");
        self.pieces[piece.color as usize][piece.kind as usize].clear(sq);
        self.colors[piece.color as usize].clear(sq);
        self.mailbox[sq as usize] = None;
        self.hash ^= piece_key(piece.color, piece.kind, sq);
        piece
    }

    /// Moves the piece on `from` to the empty square `to`.
    fn move_piece(&mut self, from: u8, to: u8) {
        let piece = self.remove_piece(from);
        self.put_piece(piece, to);
    }

    /// Replaces the castling rights, keeping the hash in sync.
    fn set_castling(&mut self, rights: u8) {
        self.hash ^= castling_key(self.castling) ^ castling_key(rights);
        self.castling = rights;
    }

    /// Replaces the en passant square, keeping the hash in sync.
    fn set_en_passant(&mut self, square: Option<u8>) {
        if let Some(old) = self.en_passant {
            self.hash ^= en_passant_key(old % 8);
        }
        if let Some(new) = square {
            self.hash ^= en_passant_key(new % 8);
        }
        self.en_passant = square;
    }

    /// `Some(ep)` if a pawn of the other side could capture onto `ep`, the square a `pusher`
    /// pawn just skipped; `None` otherwise. Looks outwards from `ep`, like `is_attacked`: a
    /// `pusher` pawn standing on `ep` would attack exactly the squares the capturers stand on.
    fn usable_en_passant(&self, ep: u8, pusher: Color) -> Option<u8> {
        let capturers = pawn_attacks(pusher, ep) & self.pieces(opponent(pusher), PieceKind::Pawn);
        if capturers.is_empty() { None } else { Some(ep) }
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

    /// Rejects castling rights whose king and rook are not on their home squares. From here
    /// on a right means "king and rook are home and never moved": `make_move` keeps that true
    /// by clearing rights as they move, so move generation can trust the flags.
    fn check_castling_rights(&self) -> Result<(), FenError> {
        for color in [Color::White, Color::Black] {
            for castle in &CASTLES[color as usize] {
                let pieces_home = self
                    .pieces(color, PieceKind::King)
                    .contains(castle.king_from)
                    && self
                        .pieces(color, PieceKind::Rook)
                        .contains(castle.rook_from);
                if self.castling & castle.right != 0 && !pieces_home {
                    return Err(FenError::CastlingWithoutPieces);
                }
            }
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

/// The other side: Black for White, White for Black.
fn opponent(color: Color) -> Color {
    match color {
        Color::White => Color::Black,
        Color::Black => Color::White,
    }
}

/// The square of the piece `mv` captures: its to square, except for en passant, where the
/// captured pawn stands one rank behind it (d5 when a white pawn takes on d6).
fn captured_square(mv: Move, us: Color) -> u8 {
    if mv.flag() == EN_PASSANT {
        (mv.to() as i8 - 8 * us.forward()) as u8
    } else {
        mv.to()
    }
}
