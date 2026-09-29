# Jira breakdown

Conventions
- **Epic** = a topic from PLAN.md. Stable; never renamed mid-project.
- **Story** = one PR. Has a "Done when" that is a test or a measurement. No story for engine internals we cannot test.
- **Subtask** = one commit. Only the commits visible now; extra commits go in CHANGELOG.md, not Jira.
- **Bug** = separate issue type, linked to its epic, closed only when a regression test passes.
- Story points are rough: 1 = an evening, 3 = a few days, 5 = a week.

Order of epics is the order of work.

---

## E1 – Project setup and tooling

**CE-1 Toolchain installed** (1)
Done when: `cargo --version`, `python --version`, `stockfish` and `cutechess-cli` all run from a terminal.
- Install rustup + stable toolchain
- Install Python 3 + PyTorch + NumPy
- Download Stockfish and cutechess-cli, note paths in README

**CE-2 Workspace skeleton** (1)
Done when: `cargo test` passes on a workspace containing the `board` crate, git has a first commit. Other crates (engine-api, engine-ab, engine-nn, uci, arena, selfplay) are added when their epic starts.
- `git init`, `.gitignore`, README stub
- `Cargo.toml` workspace with an empty `board` crate

**CE-3 Rust basics done** (3)
Done when: Rust Book chapters 1–10 read; a small scratch program using structs, enums, traits, `u64` bit ops, and a unit test compiles.
- Read chapters 1–5
- Read chapters 6–10
- Scratch program + test

---

## E2 – Board and move generation

**CE-4 Board interface and library adapter** (3) — *closed, superseded*
Closed 2026-09-29 without an adapter: move generation is our own (PLAN.md §2), so there is no library to hide. The types, the `Position` API and the FEN round-trip test were built in CE-7; `legal_moves()` and perft are in CE-8.
Original done when: the `board` crate exposes our own `Position`, `Move`, `legal_moves()`, `make()/unmake()`, piece bitboards, and `hash()`; nothing outside the crate can see the underlying library; perft to depth 4 matches reference numbers through our interface.
- Define `Square`, `Piece`, `Color`, `Move` (u16 encoding) types
- Define the `Position` API
- Adapter over the library
- FEN parse/print round-trip test (creates `board/tests/`, the first integration test)
- Perft test (start position depth 1–4)

**CE-5 Bitboard core** (3)
Done when: `Bitboard(u64)` type with set/clear/test/iterate/print, file/rank/diagonal masks, king/knight/pawn attack tables; all covered by unit tests.
- `Bitboard` type and bit iteration
- Bitwise operators for `Bitboard`: `&`, `|`, `^`, `!`, `&=`, `|=`, `^=`, `<<`, `>>`
- Precomputed masks
- King and knight tables
- Pawn attack and push tables

**CE-6 Sliding attacks** (5)
Done when: rook/bishop/queen attacks from ray-walking and from magic bitboards agree on 10,000 random occupancies.
- Ray-walking attacks
- Magic number generation
- Magic attack tables
- Cross-test ray vs magic

**CE-7 Own position, make/unmake, pseudo-legal moves** (5)
Done when: FEN round-trips; make followed by unmake restores the position and hash exactly on 10,000 random move sequences.
- `Position` with 12 bitboards + state
- Zobrist hashing
- Pseudo-legal move generation incl. castling, en passant, promotions
- Make/unmake with undo record
- Round-trip test

**Bug CE-256 FEN handling like Stockfish** — *branch `CE-256-align-fen-with-stockfish`, merged before CE-8*
Done when: `from_fen` accepts and cleans up the same positions as Stockfish's `Position::set`, and every FEN that used to crash or corrupt the position has a regression test. `from_fen` rejects more than 8 pawns, more pieces than promotions explain, a king the side to move could capture, and counters out of range; it drops castling rights without their king and rook, and en passant squares without a legal en passant capture (`make_move` follows the same en passant rule after a double push, so the same position always has the same hash and repetitions count correctly). Before the fix, such FENs panicked (en passant with no pawn, a captured king, a 256-move list overflow, counter overflow) or silently corrupted the position (en passant onto an occupied square).

**CE-8 Legality and perft gate** (5)
Done when: perft matches on all reference positions (start, Kiwipete, positions 3–6) to depth 5; move lists equal the `chess` crate oracle (a dev-dependency, used only in tests) on 10,000 random positions; nodes/second recorded in the PR description (copied into CHANGELOG when it starts in CE-12).
- Legal filter (king-attacked check)
- Perft with divide output
- Reference position tests
- Oracle comparison test

**CE-257 Game-over rules** (3)
Done when: tests on hand-picked positions and move sequences detect checkmate, stalemate, the 50-move rule, threefold repetition and insufficient material, all as queries on `Position`.
- `in_check`, checkmate and stalemate (built on CE-8's legal moves)
- 50-move rule (from `halfmove_clock`)
- Hash history in `Position` and threefold repetition (the en passant rule it relies on is done in the FEN bug fix CE-256: a position whose only en passant captures are illegal has the same hash as without the square): the start position counts as the first occurrence, then every position reached by a move; anything before the start position is unknown and never counted
- Insufficient material

---

## E3 – Engine API, UCI, Lichess

**CE-9 Engine trait and random engine** (1)
Done when: `Engine` trait exists; a random-move engine implements it; a test plays it against itself to game end without panicking.
- `Engine` trait: `set_limits(limits)`, `get_moves(pos, n, limits)`, `get_eval(pos)`; the engine keeps no game, the caller passes the position (its history shows repetitions); stopping a running search is decided in CE-10
- `SearchLimits` (depth, nodes, time); the default, used by every engine when nobody gives limits, is 1 second per move
- Random engine (its own crate, `engine-random`) + self-play test

**CE-10 UCI binary** (3)
Done when: Cute Chess GUI can load the binary and play a full game; `uci`, `isready`, `ucinewgame`, `position`, `go` (depth/movetime/wtime/btime), `stop`, `quit` handled; `stop` interrupts a running search.
- stdin/stdout loop and command parsing
- Search on a worker thread with a stop flag
- `info` lines (depth, score, nodes, pv)
- Manual GUI test documented in README

**CE-11 Lichess bot online** (1)
Done when: the random engine completes one rated or casual game on Lichess as a BOT account.
- Create BOT account and token
- Configure lichess-bot with the UCI binary
- Play one game, link it in README

---

## E4 – Engine A: goal-oriented search

Every story here ends with: bench signature recorded, puzzle set run, and a 200-game fast match vs the previous version recorded in CHANGELOG.

**CE-12 Material eval + minimax** (1)
Done when: fixed-depth search returns a legal move; a test proves depth-3 minimax finds a mate-in-1.
- `CHANGELOG.md` created with the one-line format
- Material evaluation
- Minimax with depth limit
- Mate-in-1 test

**CE-13 Alpha-beta** (1)
Done when: same best moves as minimax at fixed depth on 50 positions, with node count reduced (both numbers in CHANGELOG).
- Alpha-beta
- Equivalence test vs minimax

**CE-14 Iterative deepening and time control** (3)
Done when: `go movetime 1000` returns within 1.1 s on 100 positions; `go wtime/btime` never loses on time in a 50-game match.
- Iterative deepening loop
- Time manager
- Stop-flag checks inside search
- Time-loss test match

**CE-15 Bench command and puzzle suite** (1)
Done when: `bench` prints a deterministic node count; a mate-in-N EPD suite runs from `cargo test` and reports solved/total.
- `bench` in the UCI binary
- EPD loader
- Puzzle test with threshold

**CE-16 Move ordering** (3)
Done when: bench nodes at fixed depth drop measurably vs CE-13; each ordering step logged separately.
- MVV-LVA capture ordering
- Killer moves
- History heuristic

**CE-17 Quiescence search** (3)
Done when: puzzle score improves and horizon-effect test positions (a fixed set of 20) are handled correctly.
- Capture-only search at leaves
- Stand-pat
- Delta pruning (optional)

**CE-18 Transposition table** (3)
Done when: bench shows fewer nodes at fixed depth; determinism test still passes; no illegal moves in 500 fast games.
- Zobrist keys wired into position (if not done in CE-7)
- TT entry, replacement scheme
- TT probe/store in search
- Best-move-from-TT ordering

**CE-19 Evaluation upgrade** (3)
Done when: Elo gain vs CE-18 in a 500-game match.
- Piece-square tables
- Mobility
- King safety
- Pawn structure

**CE-20 A\* mate finder** (5)
Done when: solves a mate-in-2/3/4 set with a reported solve rate and node counts; documented as a goal-directed search with explicit g, h, and goal test.
- Node/state representation and priority queue
- Heuristic (king safety + material)
- Goal test (checkmate)
- Puzzle set test
- Write-up notes

**CE-21 Engine A milestone match** (1)
Done when: 200 games vs Stockfish at UCI_Elo 1500 recorded, result in README.
- Tournament run
- Results table

---

## E5 – Parallelism

**CE-22 Baselines** (1)
Done when: single-thread nodes/second, time-to-depth, and Elo vs Stockfish recorded in CHANGELOG as the reference row.
- Benchmark script
- Record results

**CE-23 Lock-free shared transposition table** (3)
Done when: TT works with N writers without locks (atomic entries, XOR trick or equivalent); determinism test at 1 thread unchanged.
- Atomic entry layout
- Store/probe without locks
- Test under concurrent access

**CE-24 Lazy SMP** (5)
Done when: search runs on 1/2/4/8 threads; no crashes or illegal moves in 500 games; speedup table recorded.
- Thread pool and per-thread search state
- Shared stop flag and time control
- Helper-thread depth offsets
- Speedup and Elo table

**CE-25 Parallel tournament runner** (3)
Done when: the arena plays N games across T threads and prints a results table; wall time scales with T.
- Game runner
- Thread pool over games
- Results aggregation

**CE-26 Parallelism write-up data** (1)
Done when: final table of nodes/sec, time-to-depth, Elo at 1/2/4/8 threads, with an explanation of sublinear scaling.

---

## E6 – Engine B: self-teaching neural network

**CE-27 Self-play data generator** (3)
Done when: `selfplay` produces a file of (position features, side to move, game result, search score) from N parallel fast games; throughput recorded.
- Feature encoding (768 + side + castling)
- Game loop with random opening moves for diversity
- Binary output format
- Parallel generation (reuses E5 thread pool)

**CE-28 Training script** (3)
Done when: `python train.py data.bin` trains a 768→256→32→1 net, prints loss per epoch, and exports `weights.bin`.
- Data loader
- Model
- Training loop
- Weight export

**CE-29 Rust inference** (3)
Done when: Rust evaluation of 1,000 positions matches PyTorch output within tolerance; inference cost per position recorded.
- Weight file loader
- Forward pass
- Equivalence test against PyTorch dump

**CE-30 Engine B = search + NN eval** (1)
Done when: Engine B is selectable in UCI and the arena; plays legal games.
- Engine B struct wiring NN eval into the shared search
- UCI option to pick engine/weights

**CE-31 Self-play iteration loop** (5)
Done when: at least 3 iterations run end to end (generate → train → play); Elo of each iteration vs fixed Engine A recorded; loss curve saved.
- Iteration script
- Elo matches per iteration
- Curve plots for the write-up

**CE-32 (stretch) Policy head + MCTS** (5)
Done when: Engine B can search with PUCT using the policy head; compared against alpha-beta + value net.

---

## E7 – Arena and Elo

**CE-33 Arena CLI** (3)
Done when: `arena play`, `arena analyze <fen>`, `arena bestmoves <fen> --n 3`, `arena tournament`, `arena elo` all work and are documented.

**CE-34 Elo pipeline** (3)
Done when: a script runs cutechess-cli matches vs Stockfish at several UCI_Elo settings and Ordo/bayeselo produces a rating table for all engines.

**CE-35 (optional) UI** (5)
Decide after E5. Done when: a person can play either engine from a board.

---

## E8 – Write-up and presentation

**CE-36 README and architecture** (3)
Done when: README has architecture diagram, how to build/run, Lichess link, and results tables.

**CE-37 Final report** (5)
Done when: the three topics each have a section with method, measurements, and what did not work.

**CE-38 Presentation/demo** (3)
Done when: live demo script: Lichess game, mate finder on a puzzle, thread speedup, NN Elo curve.
