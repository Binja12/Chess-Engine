//! # Position and FEN (CE-7, first subtask)
//!
//! **What is tested.** `Position::from_fen` / `to_fen` and the read-only getters, through the
//! crate's public API only.
//! - **3a, parsing:** the start position has the expected pieces and state, and on every
//!   reference position the bitboards and the mailbox agree.
//! - **3b, writing:** parsing then writing gives back the exact same FEN (round-trip), and a
//!   4-field FEN gets the default counters `0 1`.
//! - **3c, errors:** each kind of bad FEN is rejected with the matching `FenError` variant.
//!
//! Run only these tests with `cargo test -p board --test position_tests`.

use board::bitboard::Bitboard;
use board::color::Color;
use board::masks::rank_mask;
use board::piece::{Piece, PieceKind};
use board::position::{
    BLACK_KINGSIDE, BLACK_QUEENSIDE, FenError, Position, START_FEN, WHITE_KINGSIDE, WHITE_QUEENSIDE,
};

/// Start position, Kiwipete, perft positions 3-6, and a position with an en passant square.
const REFERENCE_FENS: [&str; 7] = [
    START_FEN,
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
];

const COLORS: [Color; 2] = [Color::White, Color::Black];
const KINDS: [PieceKind; 6] = [
    PieceKind::Pawn,
    PieceKind::Knight,
    PieceKind::Bishop,
    PieceKind::Rook,
    PieceKind::Queen,
    PieceKind::King,
];

/// Parses a FEN that the test knows is valid.
fn parse(fen: &str) -> Position {
    Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen:?} should parse, got {e:?}"))
}

// ---------- 3a. parsing ----------

// 1. start position: pieces and state
#[test]
fn start_position_has_expected_pieces_and_state() {
    let pos = parse(START_FEN);
    assert_eq!(pos.pieces(Color::White, PieceKind::Pawn), rank_mask(1));
    assert_eq!(pos.pieces(Color::Black, PieceKind::Pawn), rank_mask(6));
    assert_eq!(
        pos.piece_at(60),
        Some(Piece {
            color: Color::Black,
            kind: PieceKind::King
        })
    );
    assert_eq!(
        pos.piece_at(3),
        Some(Piece {
            color: Color::White,
            kind: PieceKind::Queen
        })
    );
    assert_eq!(pos.piece_at(28), None);
    assert_eq!(pos.side_to_move(), Color::White);
    assert_eq!(
        pos.castling(),
        WHITE_KINGSIDE | WHITE_QUEENSIDE | BLACK_KINGSIDE | BLACK_QUEENSIDE
    );
    assert_eq!(pos.en_passant(), None);
    assert_eq!(pos.halfmove_clock(), 0);
    assert_eq!(pos.fullmove_number(), 1);
}

// 1b. the other fields are read, not just defaulted
#[test]
fn state_fields_are_read_from_the_fen() {
    let pos = parse("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R b Kq e3 7 42");
    assert_eq!(pos.side_to_move(), Color::Black);
    assert_eq!(pos.castling(), WHITE_KINGSIDE | BLACK_QUEENSIDE);
    assert_eq!(pos.en_passant(), Some(20)); // e3
    assert_eq!(pos.halfmove_clock(), 7);
    assert_eq!(pos.fullmove_number(), 42);
}

// 2. bitboards, color sets and mailbox all agree
#[test]
fn bitboards_and_mailbox_agree() {
    for fen in REFERENCE_FENS {
        let pos = parse(fen);
        let white = pos.color_pieces(Color::White);
        let black = pos.color_pieces(Color::Black);
        assert!((white & black).is_empty(), "{fen}: colors overlap");
        assert_eq!(white | black, pos.occupied(), "{fen}: colors != occupied");

        let mut all = Bitboard::EMPTY;
        for color in COLORS {
            let mut own = Bitboard::EMPTY;
            for kind in KINDS {
                own |= pos.pieces(color, kind);
            }
            assert_eq!(own, pos.color_pieces(color), "{fen}: {color:?} pieces");
            all |= own;
        }
        assert_eq!(all, pos.occupied(), "{fen}: 12 bitboards != occupied");

        for sq in 0..64 {
            match pos.piece_at(sq) {
                Some(p) => assert!(
                    pos.pieces(p.color, p.kind).contains(sq),
                    "{fen}: mailbox has {p:?} on {sq}, bitboard does not"
                ),
                None => assert!(
                    !pos.occupied().contains(sq),
                    "{fen}: mailbox empty on {sq}, bitboards are not"
                ),
            }
        }
    }
}

// 2b. `parse::<Position>()` does the same as `from_fen`
#[test]
fn from_str_matches_from_fen() {
    let parsed: Position = START_FEN.parse().expect("start FEN parses");
    assert_eq!(parsed, parse(START_FEN));
}

// ---------- 3b. writing ----------

// 3. parse then write gives the same text
#[test]
fn reference_fens_round_trip() {
    for fen in REFERENCE_FENS {
        assert_eq!(parse(fen).to_fen(), fen);
    }
}

// 4. 4-field FEN (no counters, as in EPD files) defaults to `0 1`
#[test]
fn four_field_fen_gets_default_counters() {
    let pos = parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -");
    assert_eq!(pos.halfmove_clock(), 0);
    assert_eq!(pos.fullmove_number(), 1);
    assert_eq!(pos.to_fen(), START_FEN);
}

// ---------- 3c. errors ----------

/// Asserts that `fen` is rejected with exactly `expected`.
fn assert_rejected(fen: &str, expected: FenError) {
    assert_eq!(Position::from_fen(fen).err(), Some(expected), "{fen:?}");
}

// 5. field count
#[test]
fn wrong_field_count_is_rejected() {
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq",
        FenError::WrongFieldCount,
    );
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0",
        FenError::WrongFieldCount,
    );
    assert_rejected(&format!("{START_FEN} 5"), FenError::WrongFieldCount);
    assert_rejected("", FenError::WrongFieldCount);
}

// 6. board field shape
#[test]
fn bad_board_shape_is_rejected() {
    // 7 ranks
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        FenError::BadRankCount,
    );
    // 9 ranks
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        FenError::BadRankCount,
    );
    // 9 squares on rank 8
    assert_rejected(
        "rnbqkbnrr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        FenError::BadRankLength,
    );
    // 7 squares on rank 7
    assert_rejected(
        "rnbqkbnr/ppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        FenError::BadRankLength,
    );
    // two digits in a row
    assert_rejected(
        "rnbqkbnr/pppppppp/44/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        FenError::BadRankLength,
    );
    // digit 9 and digit 0
    assert_rejected(
        "rnbqkbnr/pppppppp/9/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        FenError::BadRankLength,
    );
    assert_rejected(
        "rnbqkbnr/pppppppp/08/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        FenError::BadRankLength,
    );
}

// 7. unknown piece letter
#[test]
fn unknown_piece_letter_is_rejected() {
    assert_rejected(
        "rnbqkbnr/ppppxppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        FenError::BadPiece('x'),
    );
}

// 8. side to move
#[test]
fn bad_side_to_move_is_rejected() {
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR z KQkq - 0 1",
        FenError::BadSideToMove,
    );
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR W KQkq - 0 1",
        FenError::BadSideToMove,
    );
}

// 9. castling field
#[test]
fn bad_castling_is_rejected() {
    for castling in ["KX", "KK", "-K", "K-", ""] {
        let fen = format!("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w {castling} - 0 1");
        // "" leaves only 5 fields, which is a field-count error, not a castling one
        let expected = if castling.is_empty() {
            FenError::WrongFieldCount
        } else {
            FenError::BadCastling
        };
        assert_rejected(&fen, expected);
    }
}

// 10. en passant field
#[test]
fn bad_en_passant_is_rejected() {
    // not a square
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e9 0 1",
        FenError::BadEnPassant,
    );
    // rank 4 is never an en passant square
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e4 0 1",
        FenError::BadEnPassant,
    );
    // rank 3 with White to move: White cannot have just double-pushed
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e3 0 1",
        FenError::BadEnPassant,
    );
    // rank 6 with Black to move
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq e6 0 1",
        FenError::BadEnPassant,
    );
}

// 11. move counters
#[test]
fn bad_clock_is_rejected() {
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - abc 1",
        FenError::BadClock,
    );
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 x",
        FenError::BadClock,
    );
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - -1 1",
        FenError::BadClock,
    );
}

// 12. each side must have exactly one king (a missing king would crash move generation)
#[test]
fn bad_king_count_is_rejected() {
    // no white king
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQ1BNR w kq - 0 1",
        FenError::BadKingCount,
    );
    // two black kings
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/3k4/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        FenError::BadKingCount,
    );
}

// 13. pawns can never stand on rank 1 or 8
#[test]
fn pawn_on_back_rank_is_rejected() {
    assert_rejected(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNP w Qkq - 0 1",
        FenError::PawnOnBackRank,
    );
    assert_rejected(
        "rnbqkbnp/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQq - 0 1",
        FenError::PawnOnBackRank,
    );
}
