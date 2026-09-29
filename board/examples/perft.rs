//! Perft from the command line, printed like Stockfish's `go perft`, to check move generation by
//! hand and to measure its speed:
//!
//! ```text
//! cargo run --release -p board --example perft -- <depth> [fen]
//! ```
//!
//! Prints one line per legal first move with the count below it (`e2e4: 9771`), sorted by move,
//! then `Nodes searched: N` and the time taken. Without a FEN it uses the start position. When a
//! total is wrong, run Stockfish's `position fen <fen>` and `go perft <depth>` on the same FEN,
//! find the move whose count differs, play it, and repeat one level shallower until the bug is
//! down to one position.

use board::perft::divide;
use board::position::{Position, START_FEN};
use std::process::exit;
use std::time::Instant;

/// Reads `<depth> [fen]`, runs `divide`, and prints the counts, the total and the speed.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let depth = match args.first().map(|text| text.parse::<u32>()) {
        Some(Ok(depth)) if depth >= 1 => depth,
        _ => {
            eprintln!("usage: perft <depth> [fen]   (depth 1 or more)");
            exit(2);
        }
    };
    // a FEN typed without quotes arrives as several arguments
    let fen = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        START_FEN.to_string()
    };
    let mut pos = match Position::from_fen(&fen) {
        Ok(pos) => pos,
        Err(e) => {
            eprintln!("bad FEN {fen:?}: {e}");
            exit(2);
        }
    };

    let start = Instant::now();
    let mut counts = divide(&mut pos, depth);
    let seconds = start.elapsed().as_secs_f64();

    counts.sort_by_key(|&(mv, _)| mv.to_string());
    for (mv, count) in &counts {
        println!("{mv}: {count}");
    }
    let nodes: u64 = counts.iter().map(|&(_, count)| count).sum();
    println!();
    println!("Nodes searched: {nodes}");
    println!(
        "Time: {seconds:.3} s ({:.1} M nodes/s)",
        nodes as f64 / seconds.max(1e-9) / 1e6
    );
}
