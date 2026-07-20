// games/mod.rs — the cartridge SELECTION seam. This is the one place in the
// crate that names a specific game module; every engine file (game.rs,
// headless.rs, render.rs, save.rs) reaches the active cartridge only through
// the re-exported `GAME` below, never by naming a game module directly —
// that's what keeps the engine grep-clean of any one game's nouns. Swapping
// which game ships is a one-line change here (or, with a compile-time
// selector for a future second cartridge, a `cfg` gate on this line — `cfg`
// is permitted here exactly as it's permitted in main.rs's backend wiring,
// since this module IS wiring, not engine logic).
// `pub(crate)` (not private): main.rs's test module reaches this cartridge's
// own named monster/item indices (RAT/GOBLIN/OGRE/...) directly for test
// fixtures — a legitimate use, since main.rs is the crate's own test-and-
// wiring surface, not engine code (it isn't in the engine grep-clean list).
pub(crate) mod contractor;
pub(crate) use contractor::GAME;
