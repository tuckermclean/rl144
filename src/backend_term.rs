// backend_term.rs — stub for the ANSI terminal backend. Task 3 replaces this
// with a real implementation that consumes the same core Cell-grid surface
// (render::render_cells) that backend_minifb.rs consumes; this stub exists
// only to prove the feature plumbing (Cargo features, compile_error!
// guards, cfg'd mod decls in main.rs) compiles and links today.

use crate::game::Game;

pub(crate) fn run(_seed0: u64, _input_log: Vec<u8>, _game: Game, _daily: bool, _day: u64) {
    eprintln!("terminal backend lands in task 3");
    std::process::exit(1);
}
