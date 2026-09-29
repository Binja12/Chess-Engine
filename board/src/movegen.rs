//! Pseudo-legal move generation: every move the side to move could make, ignoring whether it
//! leaves its own king in check (that filter is CE-8).
//!
//! There is one entry point, [`generate_moves`]. Its [`MoveGen`] config says which moves to
//! produce: which piece kinds, noisy moves (captures and promotions) and/or quiet moves, and which
//! destination squares. Engines combine these in their own crates without changing this one: a
//! search asks for [`MoveGen::CAPTURES`] first and for [`MoveGen::QUIETS`] only if it still needs
//! them, while perft and UCI use [`MoveGen::ALL`].

use crate::attacks::{
    bishop_attacks, king_attacks, knight_attacks, pawn_double_pushes, pawn_pushes, queen_attacks,
    rook_attacks,
};
use crate::bitboard::Bitboard;
use crate::castling::CASTLES;
use crate::color::Color;
use crate::masks::{FILE_A, FILE_H, RANK_1, RANK_8};
use crate::moves::{
    BISHOP_PROMOTION, BISHOP_PROMOTION_CAPTURE, CAPTURE, DOUBLE_PAWN_PUSH, EN_PASSANT,
    KNIGHT_PROMOTION, KNIGHT_PROMOTION_CAPTURE, Move, MoveList, QUEEN_PROMOTION,
    QUEEN_PROMOTION_CAPTURE, QUIET, ROOK_PROMOTION, ROOK_PROMOTION_CAPTURE,
};
use crate::piece::PieceKind;
use crate::position::Position;

/// Every square of the board.
const ALL_SQUARES: Bitboard = Bitboard { bits: u64::MAX };

/// Which moves [`generate_moves`] produces: a filter with three parts, and a move is generated
/// only if it passes all three.
///
/// | Field | A move passes when |
/// |---|---|
/// | [`pieces`](MoveGen::pieces) | the kind of the piece that moves is selected (castling is a king move) |
/// | [`noisy`](MoveGen::noisy) / [`quiet`](MoveGen::quiet) | it is noisy and `noisy` is set, or it is quiet and `quiet` is set |
/// | [`targets`](MoveGen::targets) | its to square is in `targets` |
///
/// No piece kinds, both `noisy` and `quiet` off, or empty `targets` all give an empty list.
///
/// # Noisy and quiet
///
/// Every move is exactly one of the two, decided by its flag (see [`crate::moves`]):
///
/// | Move | Flag | Class |
/// |---|---|---|
/// | move onto an empty square, pawn push | [`QUIET`] | quiet |
/// | pawn double push | [`DOUBLE_PAWN_PUSH`] | quiet |
/// | castling, short / long | [`KING_CASTLE`](crate::moves::KING_CASTLE) / [`QUEEN_CASTLE`](crate::moves::QUEEN_CASTLE) | quiet |
/// | capture | [`CAPTURE`] | noisy |
/// | en passant | [`EN_PASSANT`] | noisy |
/// | promotion, with or without a capture (4 moves: knight, bishop, rook, queen) | `*_PROMOTION`, `*_PROMOTION_CAPTURE` | noisy |
///
/// Promotions are noisy even without a capture: like a capture, they change the material on the
/// board, so a quiescence search must look at them.
///
/// # Building a config
///
/// Start from a preset:
///
/// | Preset | Moves | Used for |
/// |---|---|---|
/// | [`MoveGen::ALL`] | every pseudo-legal move | perft, checking UCI moves, the random engine, mate and stalemate |
/// | [`MoveGen::CAPTURES`] | noisy moves | quiescence search, first stage of move ordering |
/// | [`MoveGen::QUIETS`] | quiet moves | last stage of move ordering |
///
/// and narrow it with the builders. Each returns a changed copy, so calls chain like a Java
/// builder, and `MoveGen` is `Copy` (a few bytes), so passing it by value costs nothing:
///
/// | Builder | Keeps only |
/// |---|---|
/// | [`only(&[kinds])`](MoveGen::only) | moves of these piece kinds |
/// | [`landing_on(squares)`](MoveGen::landing_on) | moves whose to square is in `squares` |
///
/// Builders never widen a config: chained calls keep what every call allows.
///
/// # Examples
///
/// ```
/// use board::masks::rank_mask;
/// use board::movegen::{MoveGen, generate_moves};
/// use board::piece::PieceKind;
/// use board::position::{Position, START_FEN};
///
/// let start = Position::from_fen(START_FEN).unwrap();
///
/// // pawn and knight moves: 16 + 4
/// let pawns_and_knights = MoveGen::ALL.only(&[PieceKind::Pawn, PieceKind::Knight]);
/// assert_eq!(generate_moves(&start, pawns_and_knights).len(), 20);
///
/// // moves onto the 4th rank: the 8 double pushes
/// let fourth_rank = MoveGen::ALL.landing_on(rank_mask(3));
/// assert_eq!(generate_moves(&start, fourth_rank).len(), 8);
///
/// // nothing to capture yet
/// assert!(generate_moves(&start, MoveGen::CAPTURES).is_empty());
/// ```
///
/// The fields are public, so a config can also be written out with struct update syntax (like
/// `{ ...ALL, noisy: false }` in TS) and read directly:
///
/// ```
/// use board::movegen::MoveGen;
/// use board::piece::PieceKind;
///
/// let pawn_pushes = MoveGen { noisy: false, ..MoveGen::ALL }.only(&[PieceKind::Pawn]);
/// assert_eq!(pawn_pushes, MoveGen::QUIETS.only(&[PieceKind::Pawn]));
/// assert!(pawn_pushes.pieces[PieceKind::Pawn as usize]);
/// assert!(!pawn_pushes.pieces[PieceKind::Knight as usize]);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MoveGen {
    /// `pieces[kind as usize]`: `true` if pieces of this kind move. Castling belongs to
    /// [`PieceKind::King`].
    pub pieces: [bool; 6],
    /// Include noisy moves: captures (en passant too) and every promotion.
    pub noisy: bool,
    /// Include quiet moves: every move that is not noisy (moves onto empty squares, pawn pushes
    /// and double pushes, castling).
    pub quiet: bool,
    /// Only moves whose to square is in this set. For en passant that is the empty en passant
    /// square the pawn lands on (the captured pawn stands beside it); for castling it is the
    /// king's landing square (g1, c1, g8 or c8).
    pub targets: Bitboard,
}

impl MoveGen {
    /// Every pseudo-legal move. For perft, checking a move sent over UCI, the random engine,
    /// and (with the legality filter) finding checkmate and stalemate.
    pub const ALL: MoveGen = MoveGen {
        pieces: [true; 6],
        noisy: true,
        quiet: true,
        targets: ALL_SQUARES,
    };
    /// Noisy moves only: captures (en passant too) and promotions. For quiescence search and the
    /// first stage of move ordering.
    pub const CAPTURES: MoveGen = MoveGen {
        noisy: true,
        quiet: false,
        ..MoveGen::ALL
    };
    /// Quiet moves only: moves onto empty squares, pawn pushes, castling. For the last stage of
    /// move ordering, when the noisy moves did not already end the search of a position.
    pub const QUIETS: MoveGen = MoveGen {
        noisy: false,
        quiet: true,
        ..MoveGen::ALL
    };

    /// Keeps only moves of the given piece kinds; the other settings stay as they are.
    ///
    /// A kind stays only if `self` already had it, so the builder never widens: chained calls
    /// keep the kinds that are in every list. An empty list selects nothing. Castling is a king
    /// move, so it stays with [`PieceKind::King`].
    ///
    /// # Examples
    ///
    /// ```
    /// use board::movegen::{MoveGen, generate_moves};
    /// use board::piece::PieceKind;
    /// use board::position::{Position, START_FEN};
    ///
    /// let start = Position::from_fen(START_FEN).unwrap();
    /// let knights = MoveGen::ALL.only(&[PieceKind::Knight]);
    /// assert_eq!(generate_moves(&start, knights).len(), 4);
    ///
    /// // chained: only the knight is in both lists
    /// let chained = MoveGen::ALL
    ///     .only(&[PieceKind::Pawn, PieceKind::Knight])
    ///     .only(&[PieceKind::Knight, PieceKind::Queen]);
    /// assert_eq!(chained, knights);
    /// ```
    pub fn only(self, kinds: &[PieceKind]) -> MoveGen {
        let mut pieces = [false; 6];
        for &kind in kinds {
            pieces[kind as usize] = self.pieces[kind as usize];
        }
        MoveGen { pieces, ..self }
    }

    /// Keeps only moves whose to square is in `targets`; the other settings stay as they are.
    ///
    /// The builder never widens: two calls keep the squares that are in both sets. The to square
    /// of en passant is the empty en passant square (the captured pawn stands beside it, not on
    /// it), and the to square of castling is where the king lands (g1, c1, g8 or c8).
    ///
    /// Typical uses: every move onto one square ("who can take on d5?"), and later (CE-8) check
    /// evasions, which must land on the checking piece or between it and the king.
    ///
    /// # Examples
    ///
    /// ```
    /// use board::bitboard::Bitboard;
    /// use board::movegen::{MoveGen, generate_moves};
    /// use board::position::Position;
    /// use board::square::square_from_name;
    ///
    /// let square = |name: &str| Bitboard::from_square(square_from_name(name).unwrap());
    ///
    /// // after 1. e4 d5: only the e4 pawn can take on d5
    /// let pos = Position::from_fen("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2")
    ///     .unwrap();
    /// let onto_d5 = generate_moves(&pos, MoveGen::ALL.landing_on(square("d5")));
    /// assert_eq!(onto_d5.len(), 1);
    /// assert_eq!(onto_d5.as_slice()[0].to_string(), "e4d5");
    ///
    /// // en passant lands on the empty e6, not on e5 where the captured pawn stands
    /// let ep = Position::from_fen("4k3/8/8/3Pp3/8/8/8/4K3 w - e6 0 1").unwrap();
    /// assert_eq!(generate_moves(&ep, MoveGen::CAPTURES.landing_on(square("e6"))).len(), 1);
    /// assert!(generate_moves(&ep, MoveGen::CAPTURES.landing_on(square("e5"))).is_empty());
    /// ```
    pub fn landing_on(self, targets: Bitboard) -> MoveGen {
        MoveGen {
            targets: self.targets & targets,
            ..self
        }
    }
}

/// Generates the pseudo-legal moves of the side to move in `pos` that `config` asks for.
///
/// **Pseudo-legal** means every move follows how its piece moves, but it may leave the mover's
/// own king in check: a pinned piece still moves, and the king may step onto an attacked square.
/// Removing those is the job of the legality filter (CE-8). Castling is the exception: it is only
/// generated when it is fully allowed (see below).
///
/// `config` decides which moves appear: a move is included only if its piece kind, its class
/// (noisy or quiet) and its to square all pass. See [`MoveGen`] for the presets, the builders
/// and exactly which moves are noisy.
///
/// # What the list contains
///
/// - Only moves of the side to move ([`Position::side_to_move`]), each once, with its exact flag
///   (see [`crate::moves`]): for example [`CAPTURE`] only when the to square holds an enemy
///   piece, [`DOUBLE_PAWN_PUSH`] for a pawn's two-square move.
/// - A pawn move onto the last rank appears 4 times, once per promotion piece (knight, bishop,
///   rook, queen); there is no version that does not promote.
/// - En passant only when the position has an en passant square ([`Position::en_passant`]).
/// - Castling, written as the king's move (`e1g1`, `e1c1`, `e8g8`, `e8c8`), only when the right
///   is still set, every square between king and rook is empty, and the king is not in check,
///   does not cross an attacked square and does not land on one. b1 / b8 must be empty but may
///   be attacked: the rook crosses it, the king does not.
///
/// The order of the moves is not part of the contract (a search reorders them anyway), but it
/// is deterministic: the same position and config always give the same list, in the same order.
///
/// # Cost
///
/// No heap allocation: the [`MoveList`] is a fixed array on the stack, returned by value. A
/// narrower config does less work: piece kinds that are not selected are skipped, and castling
/// only runs its attack tests after the cheap checks (right, empty squares, target) pass.
///
/// # Panics
///
/// Panics if more than [`MAX_MOVES`](crate::moves::MAX_MOVES) (256) moves match `config`. That
/// does not happen in real games: the most legal moves any known position has is 218, and that
/// position has exactly 218 pseudo-legal moves too. [`Position::from_fen`] only accepts
/// material a real game can have (Stockfish's check; its move lists have the same 256 limit),
/// so no FEN can go over either. Before that check, invented FENs with many extra queens did
/// (27 queens can reach 279 moves).
///
/// Debug builds also panic when a castling right is set but its king or rook is not on its home
/// square. `from_fen` never allows that, so it would mean a bug in code that changed the
/// position (a `make_move` that forgot to clear a right).
///
/// # Examples
///
/// All moves of the start position, printed in UCI form:
///
/// ```
/// use board::movegen::{MoveGen, generate_moves};
/// use board::position::{Position, START_FEN};
///
/// let start = Position::from_fen(START_FEN).unwrap();
/// let moves = generate_moves(&start, MoveGen::ALL);
/// assert_eq!(moves.len(), 20);
/// for mv in moves.as_slice() {
///     println!("{mv}"); // g1f3, e2e4, ...
/// }
/// ```
///
/// Staged generation, as a search uses it: the noisy moves first, the quiet ones only if they
/// are still needed. Together they are exactly all moves (Kiwipete: 8 + 40 = 48):
///
/// ```
/// use board::movegen::{MoveGen, generate_moves};
/// use board::position::Position;
///
/// let kiwipete = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
/// let pos = Position::from_fen(kiwipete).unwrap();
///
/// let noisy = generate_moves(&pos, MoveGen::CAPTURES);
/// assert!(noisy.as_slice().iter().all(|m| m.is_capture() || m.promotion().is_some()));
/// // ...search them; only if none of them ends the search:
/// let quiet = generate_moves(&pos, MoveGen::QUIETS);
///
/// assert_eq!((noisy.len(), quiet.len()), (8, 40));
/// assert_eq!(generate_moves(&pos, MoveGen::ALL).len(), 48);
/// ```
///
/// The king's moves, castling included (Kiwipete: Kd1, Kf1, O-O-O and O-O):
///
/// ```
/// use board::movegen::{MoveGen, generate_moves};
/// use board::piece::PieceKind;
/// use board::position::Position;
///
/// let kiwipete = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
/// let pos = Position::from_fen(kiwipete).unwrap();
/// let king = generate_moves(&pos, MoveGen::ALL.only(&[PieceKind::King]));
///
/// let mut uci: Vec<String> = king.as_slice().iter().map(|m| m.to_string()).collect();
/// uci.sort();
/// assert_eq!(uci, ["e1c1", "e1d1", "e1f1", "e1g1"]);
/// ```
///
/// Pseudo-legal: the knight on e2 is pinned (moving it would expose the king on e1 to the rook
/// on e8), yet all 6 of its moves are generated; the legality filter removes them later:
///
/// ```
/// use board::movegen::{MoveGen, generate_moves};
/// use board::piece::PieceKind;
/// use board::position::Position;
///
/// let pinned = Position::from_fen("4r1k1/8/8/8/8/8/4N3/4K3 w - - 0 1").unwrap();
/// let knight = generate_moves(&pinned, MoveGen::ALL.only(&[PieceKind::Knight]));
/// assert_eq!(knight.len(), 6);
/// ```
pub fn generate_moves(pos: &Position, config: MoveGen) -> MoveList {
    let mut list = MoveList::new();
    for kind in [
        PieceKind::Knight,
        PieceKind::Bishop,
        PieceKind::Rook,
        PieceKind::Queen,
    ] {
        if config.pieces[kind as usize] {
            piece_moves(pos, kind, config, &mut list);
        }
    }
    if config.pieces[PieceKind::King as usize] {
        piece_moves(pos, PieceKind::King, config, &mut list);
        castling_moves(pos, config, &mut list);
    }
    if config.pieces[PieceKind::Pawn as usize] {
        pawn_moves(pos, config, &mut list);
    }
    list
}

// ---------- helpers ----------

/// Moves of a knight, bishop, rook, queen or king: its attack set minus its own pieces.
fn piece_moves(pos: &Position, kind: PieceKind, config: MoveGen, list: &mut MoveList) {
    let us = pos.side_to_move();
    let occ = pos.occupied();
    let enemy = occ & !pos.color_pieces(us);
    let capture_squares = if config.noisy {
        enemy & config.targets
    } else {
        Bitboard::EMPTY
    };
    let quiet_squares = if config.quiet {
        !occ & config.targets
    } else {
        Bitboard::EMPTY
    };
    for from in pos.pieces(us, kind) {
        // one piece at a time
        let attacks = match kind {
            // this piece's attack set
            PieceKind::Knight => knight_attacks(from),
            PieceKind::Bishop => bishop_attacks(from, occ),
            PieceKind::Rook => rook_attacks(from, occ),
            PieceKind::Queen => queen_attacks(from, occ),
            PieceKind::King => king_attacks(from),
            PieceKind::Pawn => unreachable!("pawns use pawn_moves"),
        };
        for to in attacks & capture_squares {
            list.push(Move::new(from, to, CAPTURE));
        }
        for to in attacks & quiet_squares {
            list.push(Move::new(from, to, QUIET));
        }
    }
}

/// Castling: we still have the right, the squares between king and rook are empty, and the king
/// is not in check, does not pass over an attacked square and does not land on one.
/// Castling is quiet.
///
/// A right is trusted to mean "king and rook are home and never moved": `from_fen` drops
/// rights without their pieces, and `make_move` clears a right when its king or rook moves or
/// the rook is captured on its corner.
/// Debug builds check this before every castling move they add.
fn castling_moves(pos: &Position, config: MoveGen, list: &mut MoveList) {
    // castling is a quiet move
    if !config.quiet {
        return;
    }
    let us = pos.side_to_move();
    let opponent = opponent(us);
    for castle in &CASTLES[us as usize] {
        // `&&` stops at the first false: the cheap bit tests come first, so the slow
        // `is_attacked` tests only run for a castle that passed them
        let allowed = pos.castling() & castle.right != 0
            && (pos.occupied() & castle.between).is_empty()
            && config.targets.contains(castle.king_to)
            && !pos.is_attacked(castle.king_from, opponent) // not out of check
            && !pos.is_attacked(castle.king_crosses, opponent) // not through check
            && !pos.is_attacked(castle.king_to, opponent); // not into check
        if allowed {
            // the right promises king and rook are home (checked in debug builds, free in release)
            debug_assert!(pos.pieces(us, PieceKind::King).contains(castle.king_from));
            debug_assert!(pos.pieces(us, PieceKind::Rook).contains(castle.rook_from));
            list.push(Move::new(castle.king_from, castle.king_to, castle.flag));
        }
    }
}

/// Every pawn move `config` asks for: pushes (quiet), then captures, promotions and en passant
/// (noisy). Pawns move set-wise: one shift moves every pawn at once and gives all the to
/// squares; each move's from square is then found by stepping back. The sets the helpers share
/// are computed once here, and each helper gets only what it needs.
fn pawn_moves(pos: &Position, config: MoveGen, list: &mut MoveList) {
    let us = pos.side_to_move();
    let pawns = pos.pieces(us, PieceKind::Pawn);
    let occ = pos.occupied();
    // one rank forward, as a square offset: +8 for White, -8 for Black
    let up = 8 * us.forward();
    let promotion_rank = match us {
        Color::White => RANK_8,
        Color::Black => RANK_1,
    };
    let single_pushes = pawn_pushes(us, pawns, occ) & config.targets;
    if config.quiet {
        let double_pushes = pawn_double_pushes(us, pawns, occ) & config.targets;
        // a push onto the last rank promotes, so it is left to the promotions
        pawn_push_moves(single_pushes & !promotion_rank, double_pushes, up, list);
    }
    if config.noisy {
        let enemies = pos.color_pieces(opponent(us)) & config.targets;
        // a capture goes one rank forward and one file sideways: every pawn is shifted at once,
        // and `step` (the same number as the shift) leads back to the from square. the edge
        // file is removed first, so the shift cannot wrap a pawn around the board.
        let towards_a = pawns & !FILE_A;
        let towards_h = pawns & !FILE_H;
        let diagonals: Diagonals = match us {
            Color::White => [(towards_a << 7, 7), (towards_h << 9, 9)],
            Color::Black => [(towards_a >> 9, -9), (towards_h >> 7, -7)],
        };
        pawn_capture_moves(diagonals, enemies & !promotion_rank, list);
        pawn_promotion_moves(
            single_pushes & promotion_rank,
            up,
            diagonals,
            enemies & promotion_rank,
            list,
        );
        if let Some(ep) = pos.en_passant()
            && config.targets.contains(ep)
        {
            pawn_en_passant_moves(diagonals, ep, list);
        }
    }
}

/// The other side: Black for White, White for Black.
fn opponent(color: Color) -> Color {
    match color {
        Color::White => Color::Black,
        Color::Black => Color::White,
    }
}

// ---------- pawn helpers ----------

/// The two diagonals pawns capture along. For each: the squares our pawns land on, and the
/// `step` (`to - from`) that leads from a landing square back to its pawn.
type Diagonals = [(Bitboard, i8); 2];

/// Quiet pawn moves, given as to squares: single pushes (none onto the last rank, those
/// promote) and double pushes. A push came from `to - up`, a double push from `to - 2 * up`.
fn pawn_push_moves(single_pushes: Bitboard, double_pushes: Bitboard, up: i8, list: &mut MoveList) {
    add_pawn_moves(single_pushes, up, QUIET, list);
    add_pawn_moves(double_pushes, 2 * up, DOUBLE_PAWN_PUSH, list);
}

/// Pawn captures that do not promote: a diagonal move onto an enemy piece in `capturable`
/// (the caller leaves out the last rank, where a capture promotes).
fn pawn_capture_moves(diagonals: Diagonals, capturable: Bitboard, list: &mut MoveList) {
    for (landing, step) in diagonals {
        add_pawn_moves(landing & capturable, step, CAPTURE, list);
    }
}

/// Promotions, 4 moves each (knight, bishop, rook, queen): pushes onto the last rank
/// (`push_squares`, one `up` ahead of their pawn) and diagonal captures onto an enemy piece on
/// the last rank (`capturable`).
fn pawn_promotion_moves(
    push_squares: Bitboard,
    up: i8,
    diagonals: Diagonals,
    capturable: Bitboard,
    list: &mut MoveList,
) {
    add_promotions(push_squares, up, PUSH_PROMOTIONS, list);
    for (landing, step) in diagonals {
        add_promotions(landing & capturable, step, CAPTURE_PROMOTIONS, list);
    }
}

/// En passant: a diagonal pawn move onto the empty en passant square `ep`. The captured pawn
/// stands on the same file one rank back (e5 for e6).
fn pawn_en_passant_moves(diagonals: Diagonals, ep: u8, list: &mut MoveList) {
    let ep_square = Bitboard::from_square(ep);
    for (landing, step) in diagonals {
        add_pawn_moves(landing & ep_square, step, EN_PASSANT, list);
    }
}

/// The 4 promotions of a pawn push: to knight, bishop, rook or queen.
const PUSH_PROMOTIONS: [u8; 4] = [
    KNIGHT_PROMOTION,
    BISHOP_PROMOTION,
    ROOK_PROMOTION,
    QUEEN_PROMOTION,
];
/// The 4 promotions of a pawn capture: to knight, bishop, rook or queen.
const CAPTURE_PROMOTIONS: [u8; 4] = [
    KNIGHT_PROMOTION_CAPTURE,
    BISHOP_PROMOTION_CAPTURE,
    ROOK_PROMOTION_CAPTURE,
    QUEEN_PROMOTION_CAPTURE,
];

/// Adds a pawn move with `flag` onto every square of `to_squares`. All these pawns moved by the
/// same `step` (`to - from`), so each one came from `to - step`.
fn add_pawn_moves(to_squares: Bitboard, step: i8, flag: u8, list: &mut MoveList) {
    for to in to_squares {
        let from = (to as i8 - step) as u8;
        list.push(Move::new(from, to, flag));
    }
}

/// Like `add_pawn_moves`, but every pawn move becomes 4 moves: one per promotion flag in `flags`.
fn add_promotions(to_squares: Bitboard, step: i8, flags: [u8; 4], list: &mut MoveList) {
    for to in to_squares {
        let from = (to as i8 - step) as u8;
        for flag in flags {
            list.push(Move::new(from, to, flag));
        }
    }
}
