# P04 — term-ffi-lint

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | M05, P03 | none | S |

**Owns:** nothing new; it changes `src/backend_term.rs` (P03).

## Purpose
`src/backend_term.rs:56-57` declares `fn read(fd: i32, buf: *mut u8, count: usize) -> isize` and
`fn write(fd: i32, buf: *const u8, count: usize) -> isize` in its `extern "C"` block. rustc 1.100.0 reports
both as `suspicious_runtime_symbol_definitions` ("suspicious definition of the runtime `read` symbol used by
the standard library": it expects `*mut c_void` and `*const c_void` for the buffer), and the Makefile's
`RUSTFLAGS="-D warnings"` turns the two warnings into errors, so `make build-term` fails on the current
stable toolchain while it passes on the gate's pinned 1.75. The window build passes on 1.100.0. The owner
made this later work, a node, not a gate change (answer Q6 on the adoption's issue #3). This node fixes the
two declarations so the terminal build is clean on both toolchains.

## Tests
No new unit test: the declarations are type-level, and the build is the test. The red target is
`PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=stable make build build-term`, which exits non-zero today
(rustc 1.100.0: two errors at `src/backend_term.rs:56` and `:57`) and 0 after. The terminal backend's existing
tests, the frame goldens and `xhash` show the backend's behaviour is unchanged.

## Changes
1. The `read` and `write` declarations in `src/backend_term.rs` take the buffer as `*mut c_void` and
   `*const c_void` (`std::ffi::c_void`, available on 1.75), with their call sites passing the buffer
   pointer cast to that type.

## Gate
- **P04-AC-1** — `PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=stable make build build-term` exits 0, on a stable rustc of 1.100.0 or later. *Verify:* that command's output before (the two `suspicious_runtime_symbol_definitions` errors) and after, recorded in the PR with `rustc --version` (the red target).
- **P04-AC-2** — The lint is fixed, not silenced: no `allow` of `suspicious_runtime_symbol_definitions` (or of a lint group containing it) is added, and `grep -rn "suspicious_runtime_symbol_definitions" src/` prints nothing. *Verify:* that grep's output and the PR's diff.
- **P04-AC-3** — The terminal backend behaves as before: the frame goldens and the cross-backend replay hash pass, and so do the terminal build's unit tests. *Verify:* `make frames`, `make xhash` and `cargo test --quiet --no-default-features --features backend-term`, all under `RUSTUP_TOOLCHAIN=1.75`, recorded in the PR.
- **P04-AC-4** — The gate passes on the PR's head commit, `msrv` included (the change builds on 1.75). *Verify:* the CI run (B04) on the PR's head commit.
- **P04-AC-5** — The stripped and packed sizes of the terminal binary are recorded before and after, with the delta and the toolchain. *Verify:* the sizes in the PR (from `make targets`).

## Risks
- "Stable" moves: a later stable rustc may add other lints that `-D warnings` turns into errors. P04-AC-1
  holds for the version recorded in the PR; a new lint is a new node, not a reopening of this one.
- The `tcgetattr`, `tcsetattr` and `ioctl` declarations are not flagged by 1.100.0; this node leaves them as
  they are.
- The signature change must not change what the calls do: the buffers and counts are the same bytes; only
  the pointer type changes.
- It waits for M05 (SPEC.md, open decision **Order of the adoption follow-ups**), because M05 moves the
  backend's test block out of the same file.
