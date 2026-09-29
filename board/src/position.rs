//! `Position`: the full state of a game at one moment, and FEN (the standard text form of it).
//!
//! Pieces are stored twice: as 12 bitboards (fast set questions: attacks, move generation)
//! and as a 64-square mailbox (fast "what is on this square?"). Both must always agree.

// the game-over rules (check, checkmate, stalemate, draws) live in position/game_over.rs
mod game_over;

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
    /// The square a pawn skipped with a double push on the last move, kept only when an en
    /// passant capture onto it is legal (Stockfish's rule: otherwise the position is the same as
    /// without it).
    en_passant: Option<u8>,
    /// Half-moves since the last capture or pawn move (50-move rule).
    halfmove_clock: u16,
    /// Starts at 1, increases after every Black move.
    fullmove_number: u16,
    /// Zobrist hash of everything except the move counters, kept up to date incrementally.
    hash: u64,
    /// The hashes of the positions before this one, oldest first: the position `from_fen` read,
    /// then one per move played since. `make_move` pushes, `unmake_move` pops. For repetitions.
    history: Vec<u64>,
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
    /// Halfmove clock above 32767, or a fullmove number past 100,000 half-moves (Stockfish's
    /// limits).
    ClockOutOfRange,
    /// A side does not have exactly one king.
    BadKingCount,
    /// A pawn stands on rank 1 or rank 8.
    PawnOnBackRank,
    /// A side has more than 8 pawns.
    TooManyPawns,
    /// A side has more pieces than promotions can explain: every knight, bishop or rook beyond
    /// 2 and every queen beyond 1 must be a promoted pawn, so it needs a missing pawn.
    TooManyPieces,
    /// The side to move attacks the other king, so it could capture it.
    KingCanBeCaptured,
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
        history: Vec::new(),
    };

    /// Parses a FEN string. Accepts 6 fields, or 4 (counters then default to `0 1`).
    ///
    /// Which positions are accepted, and how they are cleaned up, follows Stockfish's
    /// `Position::set`, so both engines see the same positions the same way:
    /// - **rejected:** text that is not a FEN, a side without exactly one king, a pawn on rank 1
    ///   or 8, more than 8 pawns or more pieces than promotions explain, a king the side to move
    ///   could capture, and counters out of range (halfmove clock over 32767, a game over
    ///   100,000 half-moves long);
    /// - **dropped:** a castling right whose king or rook is not on its home square, and an en
    ///   passant square without a legal en passant capture onto it (the same rule `make_move`
    ///   follows, so the same position always gets the same hash);
    /// - a fullmove number of 0 is read as 1.
    ///
    /// Stricter than Stockfish only on text: exactly 4 or 6 fields, counters must be numbers,
    /// no two digits in a row in a rank, `KQkq` castling letters each at most once.
    pub fn from_fen(fen: &str) -> Result<Position, FenError> {
        let fields: Vec<&str> = fen.split_whitespace().collect();
        if fields.len() != 4 && fields.len() != 6 {
            return Err(FenError::WrongFieldCount);
        }
        // the text first, field by field, in Stockfish's order
        let mut pos = Position::EMPTY;
        pos.parse_board(fields[0])?;
        pos.side_to_move = parse_side(fields[1])?;
        let castling = parse_castling(fields[2])?;
        let en_passant = parse_en_passant(fields[3], pos.side_to_move)?;
        if fields.len() == 6 {
            pos.halfmove_clock = parse_halfmove_clock(fields[4])?;
            pos.fullmove_number = parse_fullmove_number(fields[5], pos.side_to_move)?;
        }
        // then the pieces, before anything below looks up a king
        pos.check_material()?;
        pos.castling = pos.rights_with_pieces_home(castling);
        // the pawn that skipped the en passant square belongs to the side that just moved
        pos.en_passant =
            en_passant.and_then(|ep| pos.usable_en_passant(ep, opponent(pos.side_to_move)));
        pos.hash ^= castling_key(pos.castling);
        if let Some(sq) = pos.en_passant {
            pos.hash ^= en_passant_key(sq % 8);
        }
        if pos.side_to_move == Color::Black {
            pos.hash ^= black_to_move_key();
        }
        pos.check_king_cannot_be_captured()?;
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

    /// The en passant target square: set only right after a double pawn push, and only when an
    /// en passant capture onto it is legal.
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
    /// to move, castling rights, en passant square (set only when an en passant capture onto it
    /// is legal, the same rule as `from_fen`), move counters and hash; the position before the
    /// move joins the history. The move is trusted: whether it leaves the mover's king in check
    /// is not tested here.
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
        self.history.push(self.hash);

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
    /// returned for it. Afterwards the position is exactly as before `make_move`, hash and
    /// history included.
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
        // the position before the move is on the board again, so it leaves the history
        let previous = self.history.pop();
        debug_assert_eq!(
            previous,
            Some(undo.hash),
            "unmake_move: the history does not match the undo record"
        );
    }

    /// True if `mv`, a move `generate_moves` produced for this position, does not leave the
    /// mover's own king attacked. Plays the move, looks, and takes it back, so the position is
    /// unchanged afterwards: `&mut self` is needed only for that moment.
    pub fn is_legal(&mut self, mv: Move) -> bool {
        let us = self.side_to_move();
        let undo = self.make_move(mv);
        let attacked = self.is_attacked(self.pieces(us, PieceKind::King).lsb(), self.side_to_move);
        self.unmake_move(mv, undo);
        !attacked
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
            FenError::ClockOutOfRange => write!(
                f,
                "FEN halfmove clock must be at most 32767 and the game at most 100000 half-moves long"
            ),
            FenError::BadKingCount => write!(f, "each side must have exactly one king"),
            FenError::PawnOnBackRank => write!(f, "pawns cannot stand on rank 1 or 8"),
            FenError::TooManyPawns => write!(f, "a side cannot have more than 8 pawns"),
            FenError::TooManyPieces => write!(
                f,
                "a side has more pieces than its missing pawns could have promoted to"
            ),
            FenError::KingCanBeCaptured => {
                write!(f, "the side to move could capture the other king")
            }
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

    /// `Some(ep)` if the other side can really capture en passant onto `ep`, the square a
    /// `pusher` pawn just skipped with a double push; `None` otherwise. Stockfish's rule, so an
    /// en passant square exists only when it changes what can be played:
    /// - the `pusher` pawn stands just past `ep`, and `ep` and the square the pawn started from
    ///   are empty (always true after `make_move`, but a FEN can claim anything);
    /// - a pawn of the other side attacks `ep`. Looks outwards from `ep`, like `is_attacked`: a
    ///   `pusher` pawn standing on `ep` would attack exactly the squares the capturers stand on;
    /// - at least one of those captures leaves the capturer's own king safe. The capture takes
    ///   two pawns off their squares at once, which can open a line no single move could (both
    ///   pawns on the king's rank), so the test uses the board as it would be after the capture.
    fn usable_en_passant(&self, ep: u8, pusher: Color) -> Option<u8> {
        let us = opponent(pusher);
        // the pusher's pawn went from one square behind `ep` to one square past it
        let pushed = (ep as i8 + 8 * pusher.forward()) as u8;
        let started = (ep as i8 - 8 * pusher.forward()) as u8;
        let occupied = self.occupied();
        if !self.pieces(pusher, PieceKind::Pawn).contains(pushed)
            || occupied.contains(ep)
            || occupied.contains(started)
        {
            return None;
        }
        let king = self.pieces(us, PieceKind::King).lsb();
        let capturers = pawn_attacks(pusher, ep) & self.pieces(us, PieceKind::Pawn);
        for from in capturers {
            // the board after `from` takes on `ep`: `from` and `pushed` empty, `ep` occupied
            let after = (occupied ^ Bitboard::from_square(from) ^ Bitboard::from_square(pushed))
                | Bitboard::from_square(ep);
            // the captured pawn is gone, so it attacks nothing any more
            let attackers = self.attackers_to(king, pusher, after) & !Bitboard::from_square(pushed);
            if attackers.is_empty() {
                return Some(ep);
            }
        }
        None
    }

    /// Every piece of color `by` that attacks `sq`, with slider lines blocked by `occupied`
    /// instead of the real board. Answers "would `sq` be attacked after this move?" without
    /// making the move. The pieces themselves come from the real board, so the caller removes
    /// any piece the move would capture.
    fn attackers_to(&self, sq: u8, by: Color, occupied: Bitboard) -> Bitboard {
        let queens = self.pieces(by, PieceKind::Queen);
        (bishop_attacks(sq, occupied) & (self.pieces(by, PieceKind::Bishop) | queens))
            | (rook_attacks(sq, occupied) & (self.pieces(by, PieceKind::Rook) | queens))
            | (knight_attacks(sq) & self.pieces(by, PieceKind::Knight))
            | (pawn_attacks(opponent(by), sq) & self.pieces(by, PieceKind::Pawn))
            | (king_attacks(sq) & self.pieces(by, PieceKind::King))
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

    /// Rejects piece sets no game can reach, in Stockfish's order: a pawn on rank 1 or 8, a side
    /// without exactly one king, more than 8 pawns, or more pieces than promotions explain
    /// (every knight, bishop or rook beyond 2 and every queen beyond 1 must be a promoted pawn,
    /// so it needs a missing pawn). This also keeps move generation safe: both kings are always
    /// there, and real-game material stays far below the 256 moves a `MoveList` holds (the
    /// known maximum is 218).
    fn check_material(&self) -> Result<(), FenError> {
        let pawns =
            self.pieces(Color::White, PieceKind::Pawn) | self.pieces(Color::Black, PieceKind::Pawn);
        if !(pawns & (RANK_1 | RANK_8)).is_empty() {
            return Err(FenError::PawnOnBackRank);
        }
        for color in [Color::White, Color::Black] {
            if self.pieces(color, PieceKind::King).count() != 1 {
                return Err(FenError::BadKingCount);
            }
        }
        for color in [Color::White, Color::Black] {
            let count = |kind| self.pieces(color, kind).count();
            let pawns = count(PieceKind::Pawn);
            if pawns > 8 {
                return Err(FenError::TooManyPawns);
            }
            let promoted = count(PieceKind::Knight).saturating_sub(2)
                + count(PieceKind::Bishop).saturating_sub(2)
                + count(PieceKind::Rook).saturating_sub(2)
                + count(PieceKind::Queen).saturating_sub(1);
            if promoted > 8 - pawns {
                return Err(FenError::TooManyPieces);
            }
        }
        Ok(())
    }

    /// `rights` without the castling rights whose king or rook is not on its home square. From
    /// here on a right means "king and rook are home and never moved": `make_move` keeps that
    /// true by clearing rights as they move, so move generation can trust the flags. Stockfish
    /// drops such rights too, except that it reads a king or rook elsewhere on the back rank as
    /// a Chess960 right; we only play standard chess.
    fn rights_with_pieces_home(&self, rights: u8) -> u8 {
        let mut kept = rights;
        for color in [Color::White, Color::Black] {
            for castle in &CASTLES[color as usize] {
                let pieces_home = self
                    .pieces(color, PieceKind::King)
                    .contains(castle.king_from)
                    && self
                        .pieces(color, PieceKind::Rook)
                        .contains(castle.rook_from);
                if !pieces_home {
                    kept &= !castle.right;
                }
            }
        }
        kept
    }

    /// Rejects a position where the side to move attacks the other king: it could capture it,
    /// so the other side's last move was illegal (Stockfish: "King can be captured").
    fn check_king_cannot_be_captured(&self) -> Result<(), FenError> {
        let them = opponent(self.side_to_move);
        let king = self.pieces(them, PieceKind::King).lsb();
        if self.is_attacked(king, self.side_to_move) {
            return Err(FenError::KingCanBeCaptured);
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

/// Largest halfmove clock a FEN may have (Stockfish's limit).
const MAX_HALFMOVE_CLOCK: u64 = 32_767;

/// Most half-moves the game may already have lasted when a FEN is read (Stockfish's limit).
const MAX_GAME_PLY: u64 = 100_000;

/// Reads the halfmove clock: a number from 0 to [`MAX_HALFMOVE_CLOCK`].
fn parse_halfmove_clock(text: &str) -> Result<u16, FenError> {
    let clock = parse_counter(text)?;
    if clock > MAX_HALFMOVE_CLOCK {
        return Err(FenError::ClockOutOfRange);
    }
    Ok(clock as u16)
}

/// Reads the fullmove number. As in Stockfish, 0 is read as 1, and the game so far may be at
/// most [`MAX_GAME_PLY`] half-moves long: `2 * (fullmove - 1)`, plus 1 when Black is to move.
fn parse_fullmove_number(text: &str, side: Color) -> Result<u16, FenError> {
    let fullmove = parse_counter(text)?.max(1);
    let ply = (fullmove - 1)
        .saturating_mul(2)
        .saturating_add((side == Color::Black) as u64);
    if ply > MAX_GAME_PLY {
        return Err(FenError::ClockOutOfRange);
    }
    Ok(fullmove as u16)
}

/// Reads a move counter: a whole number, not negative.
fn parse_counter(text: &str) -> Result<u64, FenError> {
    text.parse::<u64>().map_err(|_| FenError::BadClock)
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
