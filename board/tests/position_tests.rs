//! # Position and FEN (CE-7, first subtask)
//!
//! **What is tested.** `Position::from_fen` / `to_fen` and the read-only getters, through the
//! crate's public API only.
//! - **3a, parsing:** the start position has the expected pieces and state, and on every
//!   reference position the bitboards and the mailbox agree.
//! - **3b, writing:** parsing then writing gives back the exact same FEN (round-trip), and a
//!   4-field FEN gets the default counters `0 1`.
//! - **3c, errors:** each kind of bad FEN is rejected with the matching `FenError` variant.
//! - **Zobrist hash (second subtask):** equal positions hash equal, each feature (piece, side,
//!   castling, en passant file) changes the hash, the move counters do not, and the hash kept by
//!   `from_fen` equals a from-scratch recomputation. Key quality is tested in `zobrist.rs`.
//!
//! Run only these tests with `cargo test -p board --test position_tests`.

use board::bitboard::Bitboard;
use board::color::Color;
use board::masks::rank_mask;
use board::piece::{Piece, PieceKind};
use board::position::{
    BLACK_KINGSIDE, BLACK_QUEENSIDE, FenError, Position, START_FEN, WHITE_KINGSIDE, WHITE_QUEENSIDE,
};

/// Start position, Kiwipete, perft positions 3-6, and a position with an en passant square
/// (1. e4 d5 2. e5 f5: the e5 pawn can take on f6).
const REFERENCE_FENS: [&str; 7] = [
    START_FEN,
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
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
    // White just played e2-e4 and the d4 pawn can take on e3
    let pos = parse("r3k2r/8/8/8/3pP3/8/8/R3K2R b Kq e3 0 42");
    assert_eq!(pos.side_to_move(), Color::Black);
    assert_eq!(pos.castling(), WHITE_KINGSIDE | BLACK_QUEENSIDE);
    assert_eq!(pos.en_passant(), Some(20)); // e3
    assert_eq!(pos.fullmove_number(), 42);
    // a halfmove clock that is not the default 0 (a double push resets it, so no en passant here)
    assert_eq!(
        parse("r3k2r/8/8/8/8/8/8/R3K2R b Kq - 7 42").halfmove_clock(),
        7
    );
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

// 9b. a castling right needs its king and that rook on their home squares; after that the
//     move generator trusts the right (make_move keeps it true by clearing rights as pieces move)
#[test]
fn castling_right_without_its_king_and_rook_is_rejected() {
    // no rooks at all
    assert_rejected(
        "4k3/8/8/8/8/8/8/4K3 w KQ - 0 1",
        FenError::CastlingWithoutPieces,
    );
    assert_rejected(
        "4k3/8/8/8/8/8/8/4K3 w kq - 0 1",
        FenError::CastlingWithoutPieces,
    );
    // king not on e1 (it is on f1)
    assert_rejected(
        "r3k2r/8/8/8/8/8/8/R4K1R w K - 0 1",
        FenError::CastlingWithoutPieces,
    );
    // king not on e8 (it is on d8)
    assert_rejected(
        "r2k3r/8/8/8/8/8/8/R3K2R w q - 0 1",
        FenError::CastlingWithoutPieces,
    );
    // a rook of the other color on the corner does not count
    assert_rejected(
        "r3k2r/8/8/8/8/8/8/r3K2R w Q - 0 1",
        FenError::CastlingWithoutPieces,
    );

    // each right is checked on its own: Q with the a1 rook is fine although h1 is empty
    let pos = parse("r3k2r/8/8/8/8/8/8/R3K3 w Q - 0 1");
    assert_eq!(pos.castling(), WHITE_QUEENSIDE);
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

// 10b. an en passant square no enemy pawn can capture onto is dropped, so the position is the
//      same as without it (same FEN, same hash): the rule make_move uses too
#[test]
fn en_passant_square_without_a_capturer_is_dropped() {
    // after 1. e4 e5 no white pawn stands beside e5, so e6 is useless
    let pos = parse("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2");
    let without = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
    assert_eq!(pos.en_passant(), None);
    assert_eq!(pos.to_fen(), without);
    assert_eq!(pos.hash(), parse(without).hash());
    // Black just played a7-a5; the h4 pawn reaches a6 only by wrapping around the board edge
    // (h4 = 31, 31 + 9 = 40 = a6), so it cannot take on a6
    assert_eq!(parse("4k3/8/8/p7/7P/8/8/4K3 w - a6 0 1").en_passant(), None);

    // with a pawn that can take (e5 takes on f6), the square is kept
    let pos = parse("rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3");
    assert_eq!(pos.en_passant(), Some(sq("f6")));
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

// ---------- Zobrist hash ----------

// 14. the same position always gets the same hash
#[test]
fn same_fen_gives_same_hash() {
    for fen in REFERENCE_FENS {
        assert_eq!(parse(fen).hash(), parse(fen).hash(), "{fen}");
    }
}

// 15. changing any single feature changes the hash
#[test]
fn each_feature_changes_the_hash() {
    let pairs = [
        // side to move
        (
            START_FEN,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1",
        ),
        // castling rights
        (
            START_FEN,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w Kkq - 0 1",
        ),
        // a piece on a different square (Ng1-f3)
        (
            START_FEN,
            "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R w KQkq - 0 1",
        ),
        // a different kind on the same square (knight -> bishop on g1)
        (
            START_FEN,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBBR w KQkq - 0 1",
        ),
        // a different color on the same square (pawn on e4)
        (
            "8/8/8/8/4P3/8/8/K6k w - - 0 1",
            "8/8/8/8/4p3/8/8/K6k w - - 0 1",
        ),
        // en passant square vs none (the e5 pawn can take on f6)
        (
            "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
            "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq - 0 3",
        ),
        // en passant on a different file (the e5 pawn can take on d6 or f6)
        (
            "4k3/8/8/3pPp2/8/8/8/4K3 w - d6 0 1",
            "4k3/8/8/3pPp2/8/8/8/4K3 w - f6 0 1",
        ),
    ];
    for (a, b) in pairs {
        assert_ne!(parse(a).hash(), parse(b).hash(), "{a} vs {b}");
    }
}

// 16. the move counters are not part of the position's identity
#[test]
fn move_counters_do_not_change_the_hash() {
    let later = parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 37 90");
    assert_eq!(later.hash(), parse(START_FEN).hash());
}

// 17. the hash built up while parsing equals a from-scratch recomputation
#[test]
fn incremental_hash_matches_recomputed_hash() {
    for fen in REFERENCE_FENS {
        let pos = parse(fen);
        assert_eq!(pos.hash(), pos.compute_hash(), "{fen}");
    }
}

// 18. no collisions among the reference positions
#[test]
fn reference_positions_have_distinct_hashes() {
    let hashes: std::collections::HashSet<u64> =
        REFERENCE_FENS.iter().map(|fen| parse(fen).hash()).collect();
    assert_eq!(hashes.len(), REFERENCE_FENS.len());
}

// ---------- is_attacked ----------

/// Square index from its name, e.g. `sq("e4")`.
fn sq(name: &str) -> u8 {
    board::square::square_from_name(name).expect("valid square name")
}

// 19. pawns attack diagonally forward, in their own direction
#[test]
fn pawns_attack_diagonally_forward() {
    let black = parse("4k3/8/8/8/3p4/8/8/4K3 w - - 0 1");
    assert!(black.is_attacked(sq("c3"), Color::Black));
    assert!(black.is_attacked(sq("e3"), Color::Black));
    assert!(
        !black.is_attacked(sq("d3"), Color::Black),
        "not straight ahead"
    );
    assert!(!black.is_attacked(sq("c5"), Color::Black), "not backwards");

    let white = parse("4k3/8/8/8/3P4/8/8/4K3 w - - 0 1");
    assert!(white.is_attacked(sq("c5"), Color::White));
    assert!(white.is_attacked(sq("e5"), Color::White));
    assert!(!white.is_attacked(sq("c3"), Color::White), "not backwards");
}

// 20. knight and king
#[test]
fn knights_and_kings_attack() {
    let pos = parse("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1");
    assert!(pos.is_attacked(sq("b3"), Color::White), "knight");
    assert!(pos.is_attacked(sq("f5"), Color::White), "knight");
    assert!(!pos.is_attacked(sq("d5"), Color::White));
    assert!(pos.is_attacked(sq("d2"), Color::White), "king");
    assert!(pos.is_attacked(sq("f1"), Color::White), "king");
    assert!(!pos.is_attacked(sq("e3"), Color::White));
}

// 21. sliders: bishop diagonals, rook lines, queen both
#[test]
fn sliders_attack_along_their_lines() {
    let bishop = parse("4k3/8/8/8/3B4/8/8/4K3 w - - 0 1");
    for name in ["a1", "h8", "g1", "a7"] {
        assert!(bishop.is_attacked(sq(name), Color::White), "bishop {name}");
    }
    assert!(!bishop.is_attacked(sq("d5"), Color::White));

    let rook = parse("4k3/8/8/8/3R4/8/8/4K3 w - - 0 1");
    for name in ["d8", "a4", "h4"] {
        assert!(rook.is_attacked(sq(name), Color::White), "rook {name}");
    }
    assert!(!rook.is_attacked(sq("e5"), Color::White));

    let queen = parse("4k3/8/8/8/3Q4/8/8/4K3 w - - 0 1");
    assert!(queen.is_attacked(sq("a1"), Color::White), "queen diagonal");
    assert!(queen.is_attacked(sq("d8"), Color::White), "queen file");
    assert!(
        !queen.is_attacked(sq("c6"), Color::White),
        "a knight jump away"
    );
}

// 22. a piece in between blocks a slider (the blocker itself is still attacked)
#[test]
fn blocked_sliders_do_not_attack_past_the_blocker() {
    let bishop = parse("4k3/8/8/8/3B4/2P5/8/4K3 w - - 0 1");
    assert!(bishop.is_attacked(sq("c3"), Color::White), "the blocker");
    assert!(!bishop.is_attacked(sq("b2"), Color::White));
    assert!(!bishop.is_attacked(sq("a1"), Color::White));

    let rook = parse("4k3/8/3p4/8/3R4/8/8/4K3 w - - 0 1");
    assert!(rook.is_attacked(sq("d6"), Color::White), "the blocker");
    assert!(!rook.is_attacked(sq("d7"), Color::White));
}

// 23. only pieces of the asked color count
#[test]
fn only_the_given_color_attacks() {
    let pos = parse("4k3/8/8/8/3R4/8/8/4K3 w - - 0 1");
    assert!(pos.is_attacked(sq("a4"), Color::White));
    assert!(!pos.is_attacked(sq("a4"), Color::Black));
}
