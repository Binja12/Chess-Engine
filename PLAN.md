# Chess Engine – Finals Project Plan

## 1. Goal

Build a chess engine system in Rust that demonstrates three course topics:

| # | Topic | Demonstrated by |
|---|-------|-----------------|
| 1 | AI – goal-oriented state-space search (מונחה מטרה) | Own bitboard state representation and move generation, then Engine A: hand-written search with alpha-beta pruning, iterative deepening, and a best-first (A*-style) mate finder |
| 2 | Neural networks that self-teach | Engine B: neural-network evaluation trained from self-play games |
| 3 | Parallelism for optimization (מקביליות) | Multi-threaded search in Engine A, parallel self-play generation for Engine B, parallel tournament runner |

Plus a top-level "arena" that drives the engines (best move, evaluation, top-N moves), speaks the standard UCI protocol so the engines can play on Lichess, and measures Elo against each other and against Stockfish.

## 2. Honest notes on the requirements

**A\* and chess.** A\* is a single-agent shortest-path algorithm: it needs a goal state and a cost-to-go heuristic. Chess is a two-player adversarial game, so the "goal-oriented state exploration" that engines actually use is minimax with alpha-beta pruning. Alpha-beta is the same family of ideas (explore a state tree, prune with bounds, order by heuristic), and it is what we will build as the core. To still show a genuine A\* in the project, Engine A will also contain a **mate finder**: a best-first search where the goal is checkmate, the cost is the number of moves, and the heuristic is king safety and material. That is a real, defensible A\* use and a nice demo (solve mate-in-N puzzles). Suggested framing for the write-up: "goal-directed search: A\* for the single-goal problem (find mate), alpha-beta for the adversarial problem (find best move)".

**"Neural network that self-teaches."** The full AlphaZero recipe (policy+value net, Monte-Carlo tree search, millions of self-play games) needs GPU-weeks. A scaled-down version that a student laptop can run: a **value network** that evaluates a position, trained on positions from **self-play games**, plugged into the same alpha-beta search as Engine A. Training loop: play games → record positions and results → train net → play with the new net → repeat. First iteration can be bootstrapped from Engine A's search scores so the net does not start from random. Stretch goal: a policy head plus MCTS (AlphaZero-lite).

**Rust without Rust knowledge.** A chess engine is mostly integers, arrays, and loops, which is the easy part of Rust. The hard parts (lifetimes, borrow checker on complex object graphs) mostly do not show up. Expect the first two weeks to be slow and then it gets fast.

**Move generation is our own.** The board representation (bitboards) and legal move generation are written from scratch as a deliberate part of the project: understanding how a position is encoded in 64-bit integers and how moves transform it is an algorithmic goal in itself, and it means every layer above (search, parallelism, network input) runs on code we fully own. Cost: roughly 2–3 weeks, and correctness must be proven with perft (exhaustive move counting against known reference numbers) before any search work starts, because a move-generation bug looks exactly like a search bug and is much harder to find later. The `chess` crate may be used only as a **test oracle**: compare our move lists against it on random positions.

**Lichess vs chess.com.** Lichess officially supports bots: create a BOT account and run the open-source `lichess-bot` bridge, which talks to any UCI engine. chess.com has no bot API and forbids engines on its site, so it is out of scope. UCI compliance is what makes the Lichess integration free.

**Elo measurement.** Elo is only meaningful relative to something. Plan: run tournaments with `cutechess-cli` (or `fastchess`) between our engines and Stockfish set to fixed strength (UCI option `UCI_LimitStrength` + `UCI_Elo`, or `Skill Level`). Stockfish's own rating at those settings gives the anchor. Our arena can also run these tournaments itself, which doubles as a parallelism demo.

## 3. Architecture

Cargo workspace with one crate per responsibility. Every engine implements the same trait, so the top level does not care which engine it is talking to.

```
chess-engine/
├── Cargo.toml                 workspace
├── crates/
│   ├── board/                 own bitboards: Position, Move, attack tables (magic bitboards), legal move gen, make/unmake, FEN, Zobrist hash, perft
│   ├── engine-api/            trait Engine { set_position, best_move(limits), evaluate, top_moves(n), stop }
│   ├── engine-ab/             Engine A: alpha-beta search + hand-written evaluation + A* mate finder
│   ├── engine-nn/             Engine B: neural-net evaluation (Rust inference), reuses engine-ab search
│   ├── uci/                   binary: wraps any Engine in the UCI protocol (stdin/stdout)
│   ├── arena/                 top level: CLI (play, analyze, bestmoves N, tournament, elo), game runner
│   └── selfplay/              parallel self-play game generator, writes training data
├── training/                  Python + PyTorch: train value net from self-play data, export weights
├── tests/                     perft, mate puzzles, EPD test suites
└── docs/                      write-up, benchmark results, Elo tables
```

Engine A and Engine B share the search code; they differ only in the evaluation function. This is deliberate: it isolates "what the network learned" from "how good the search is", which makes the comparison in the write-up clean.

Rust inference for the network is a few matrix multiplies written by hand (or the `candle` crate). Training stays in Python because that is where the tooling is; the net is small enough to export as a flat weights file.

## 4. Phases

Each phase ends with something runnable and testable. Rough durations assume part-time work.

### Phase 0 – Setup and Rust basics (1 week)
- Install Rust (rustup), Python 3, PyTorch, Stockfish, cutechess-cli.
- Read The Rust Book chapters 1–10 (ownership, structs, enums, collections, error handling, traits). Bit manipulation in Rust: `u64`, shifts, `count_ones`, `trailing_zeros`.
- Create the workspace and an empty `board` crate.
- Done when: `cargo test` runs in the workspace.

### Phase 1 – Board and move generation, from scratch (2–3 weeks)
Build in this order; each step has its own tests.
1. **Bitboard basics**: `Bitboard(u64)` type, square indexing (a1 = 0 … h8 = 63), set/clear/test bit, iterate set bits, pretty-print. Precomputed masks: files, ranks, diagonals.
2. **Position struct**: 12 piece bitboards (6 piece types × 2 colors), occupancy by color, side to move, castling rights, en passant square, halfmove clock, fullmove number. FEN parse and print, round-trip tested.
3. **Non-sliding attacks**: lookup tables for king, knight, and pawn attacks (64 entries each, computed at startup or as `const`).
4. **Sliding attacks** (rook, bishop, queen): first a simple ray-walking version so everything else can proceed, then **magic bitboards** as the algorithmic centrepiece: precomputed attack tables indexed by (square, hashed relevant occupancy). Both versions kept and cross-tested.
5. **Pseudo-legal move generation**: all moves ignoring whether the own king is left in check. Compact `Move` encoding in a `u16`/`u32` (from, to, promotion, flags).
6. **Make / unmake move**: incremental bitboard updates, castling, en passant, promotion, Zobrist hash kept in sync. Unmake restores state from a saved "undo" record.
7. **Legality**: filter pseudo-legal moves by "is own king attacked after the move" (simple first), later optimised with pin and check masks.
8. **Perft**: count leaf nodes to depth N and compare against reference values for the standard test positions (start position, "Kiwipete", and the other well-known perft positions). Also compare move lists against the `chess` crate on random positions as an oracle.
- Done when: perft matches on all reference positions to depth 5 or 6, and nodes/second is measured and recorded (baseline for Phase 4).

### Phase 2 – Skeleton: API, UCI, random mover (1 week)
- `engine-api` trait.
- `engine-random`: picks a random legal move. Trivial, but it proves the whole pipeline.
- `uci` binary: implements `uci`, `isready`, `position`, `go`, `stop`, `quit`. Test with a GUI (Cute Chess or Arena GUI).
- Put it on Lichess via `lichess-bot`. Early win, and the integration is settled before the hard work starts.
- Done when: the random engine plays a full game on Lichess.

### Phase 3 – Engine A: goal-oriented search (3–4 weeks)
Build up in steps, measuring strength after each one (see Phase 6 tooling; a quick version exists from the start):
1. Material-only evaluation + plain minimax, depth 3.
2. Alpha-beta pruning. Show node-count drop vs minimax in the write-up.
3. Iterative deepening with time control.
4. Move ordering: captures first (MVV-LVA), killer moves, history heuristic. Show effect on pruning.
5. Quiescence search (avoid the horizon effect).
6. Transposition table keyed by Zobrist hash.
7. Better evaluation: piece-square tables, mobility, king safety, pawn structure.
8. A\* mate finder: best-first search over the game tree with goal = checkmate, g = plies, h = heuristic. Validate on a mate-in-2/3/4 puzzle set.
- Done when: Engine A beats Stockfish at ~1500 Elo setting more often than not, and solves a mate-in-3 puzzle set.

### Phase 4 – Parallelism (2–3 weeks)
- Establish single-thread baselines first: nodes/second, time-to-depth, Elo. Without a baseline there is nothing to show.
- Lazy SMP: N threads search the same position sharing a lock-free transposition table (atomic entries). This is what Stockfish does and it is surprisingly simple.
- Alternative or addition: root-move splitting with `rayon`.
- Parallel self-play game generation (needed in Phase 5 anyway).
- Parallel tournament runner in `arena`.
- Measure speedup and Elo gain at 1, 2, 4, 8 threads. Explain why speedup is not linear (search overhead, TT contention).
- Done when: benchmark table and Elo table at several thread counts.

### Phase 5 – Engine B: self-teaching neural network (3–4 weeks)
- Input encoding: 768 features (12 piece types × 64 squares), side to move, castling rights. Output: one number, expected game result from the side to move's view.
- Network: small fully connected net (768 → 256 → 32 → 1). Small enough for fast CPU inference inside the search.
- Data: `selfplay` crate plays thousands of fast games in parallel, records (position, game result, search score).
- Training (Python): supervised on the recorded data. Iteration 0 uses Engine A's scores as labels (bootstrap). Iterations 1+ use games played by Engine B itself, so the net is learning from its own play.
- Export weights to a flat file; Rust loads and evaluates.
- Track: loss per iteration, Elo of Engine B per iteration vs a fixed Engine A. A rising curve is the demo.
- Stretch: policy head + MCTS (AlphaZero-lite).
- Done when: Engine B with a trained net measurably outperforms Engine B with random weights, and ideally approaches Engine A.

### Phase 6 – Arena, Elo, write-up (2 weeks, but tooling starts in Phase 2)
- `arena` CLI commands: `play <engine>`, `analyze <fen>`, `bestmoves <fen> --n 3`, `tournament <engines...> --games N --threads T`, `elo` report.
- Elo pipeline: cutechess-cli tournaments vs Stockfish at several fixed Elo settings, ratings computed with Ordo or bayeselo. Cross-check with the arena's own tournament runner.
- Optional UI: a simple web board (any JS chessboard library) talking to the arena over UCI or a tiny HTTP layer. Decide after Phase 4; Lichess already gives a UI for free.
- Write-up: architecture, the three topics with measurements, what worked and what did not.

## 5. Working method (how we avoid stale plans and recurring bugs)

This plan fixes only stable things: phases, crate interfaces, the protocol, and each phase's "done when". Engine internals will change constantly; that is expected and is not tracked as stories.

- **Git from day one.** Small commits. Commit message = what changed + measured result.
- **`CHANGELOG.md`, one line per change**: date, what, bench signature, Elo/perft/puzzle result. This is the record of "did I already do this", not a ticket tracker. Started with Engine A (CE-12), when algorithm experiments begin.
- **Issue tracker is for bugs only.** A bug is closed only when a test reproduces it and passes.
- **Always-on regression tests**, run before every commit:
  - perft on the reference positions (move generation),
  - a mate-in-N puzzle set (search correctness),
  - `bench`: fixed positions searched to fixed depth, printing total node count. Any change to search or evaluation changes this number, so unintended behavior changes are caught immediately,
  - fixed-depth determinism: same position and depth must give the same node count on every run and thread count (catches races in Phase 4).
- **Measure before opinion.** A search or evaluation change is kept only if fast games show an Elo gain; otherwise it is reverted. This loop (change → bench → games → keep/revert) replaces re-planning.
- **Re-plan only at phase boundaries**, ~30 minutes, based on the measurements, adjusting only the next phase.

## 6. Tooling and dependencies

| Need | Choice |
|------|--------|
| Language | Rust (stable), cargo workspace |
| Move generation | own bitboards + magic bitboards in the `board` crate; `chess` crate used only as a test oracle |
| Parallelism | `std::thread` + atomics for Lazy SMP, `rayon` for data-parallel work |
| NN training | Python 3, PyTorch, NumPy |
| NN inference | hand-written matmul in Rust (or `candle`) |
| Protocol | UCI |
| Testing / GUI | Cute Chess GUI, cutechess-cli |
| Reference engine | Stockfish (free, UCI) |
| Online play | Lichess BOT account + `lichess-bot` |
| Rating | Ordo or bayeselo over PGN results |

## 7. Risks

- **Rust learning curve** eats Phase 0–1. Mitigation: bitboards are mostly `u64` arithmetic, which is the friendliest corner of Rust; tiny steps with a test per step.
- **Move-generation bugs leaking into search.** Mitigation: perft on all reference positions must pass before Phase 3 starts; the `chess` crate as an oracle on random positions.
- **NN too weak to be interesting.** Mitigation: bootstrap from Engine A, keep the net small, judge success by the improvement curve, not by absolute strength.
- **Parallel search bugs** (data races look like random weak play). Mitigation: keep single-thread mode always available, compare results at fixed depth.
- **Scope creep** (GUI, MCTS, advanced legality optimisations). All are marked stretch; core deliverable does not depend on them.

## 8. Open decisions

1. Deadline date, and hours per week available. This sets which stretch goals are realistic.
2. ~~Move generation~~ Decided: written from scratch with bitboards (Phase 1).
3. UI: none / terminal / web. Recommended: decide after Phase 4.
4. Hardware: CPU core count and whether a GPU is available (affects how much self-play and training is feasible).
