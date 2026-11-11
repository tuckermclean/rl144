# B01 — term-ioctl-32bit

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | T02 | S01 | S |

**Owns:** `src/backend_term.rs` (the `ioctl` declaration, `TIOCGWINSZ` and its comment), `Makefile` (new `i686` target, outside `check`), `.github/workflows/check.yml` (a second job), `tests/term-pty.test.sh` (new), `CLAUDE.md` ("Verification": the `make i686` command)
**Decisions:** OD9

## Purpose
The camera console (`f33a80e`) was built for the 486's 80×25 console, but it never engages on
32-bit. `backend_term.rs:80` declares `fn ioctl(fd: i32, request: u64, argp: *mut Winsize)`;
the C prototype is variadic with an `unsigned long` (glibc) or `int` (musl) request. On i386
cdecl the `u64` takes two stack slots, libc reads the zero high half as `argp`, the call fails
with `EFAULT`, `term_size()` returns `None` and `show()` falls back to 80×30. An i686-musl run in
a 40×20 pty showed `ioctl(1, TIOCGWINSZ, NULL) = -1 EFAULT` and rows addressed up to 30; x86_64
works (audit #74, §5.1, both confirmed by a second reviewer). The 11 camera unit tests pass
because they test only the pure viewport. This node fixes the declaration (answer Q1) and adds
the 32-bit build, test and pty check the repository never had, as its own make target and CI
job, so `make check` stays host-only (principle 3).

## Tests
Keep every existing test. Add `tests/term-pty.test.sh <binary>`: it runs the given term binary in
a 40-column, 20-row pseudo-terminal (`script` from util-linux, `stty rows 20 cols 40`), feeds it
input that leaves the title and quits, and fails if any cursor-position escape in the output
addresses a row above 20 or a column above 40. Red target: `make i686`, which builds the
i686-unknown-linux-musl term binary and runs the script on it; it exits non-zero at the base
(the camera never engages) and 0 after.

## Changes
1. Declare `ioctl` as `fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int` (variadic, from
   `core::ffi`), with `TIOCGWINSZ: c_ulong`. *B01-AC-1, B01-AC-3.*
2. Correct the comment on `TIOCGWINSZ`: `0x5413` holds on x86, x86_64 and ARM Linux, not on
   every architecture. *B01-AC-4.*
3. Add a `make i686` target (not a prerequisite of `check`): add the target with rustup if
   missing, build the term backend for `i686-unknown-linux-musl`, run its `cargo test`, replay
   `tests/fixtures/ref.sav` and require the x86_64 hash, compare `--dump` for the golden seeds
   with `tests/golden/`, and run `tests/term-pty.test.sh` on the binary. *B01-AC-1, B01-AC-5.*
4. Add an `i686` job to `.github/workflows/check.yml` that installs what `make i686` needs
   (`gcc-multilib` or a musl cross linker, `util-linux`) and runs it. *B01-AC-6.*
5. Name `make i686` and what it checks in `CLAUDE.md`'s "Verification" section (the "Building
   a node" section already lists it).
   *B01-AC-7.*

## Gate
- **B01-AC-1** — On the i686-unknown-linux-musl term build, the console fits a 40×20 terminal: `tests/term-pty.test.sh` passes (no cursor row above 20 or column above 40), and the same script fails on the base commit's i686 build. *Verify:* the CI `i686` job log at the head, and a run of the script against the base's i686 binary (recorded log or run URL).
- **B01-AC-2** — On the x86_64 term build the same pty check passes. *Verify:* `sh tests/term-pty.test.sh target/term/release/rl144` in the CI log.
- **B01-AC-3** — `src/backend_term.rs` declares `ioctl` variadic with a `c_ulong` request and no `u64` request remains. *Verify:* `grep -n 'fn ioctl' src/backend_term.rs` shows `request: c_ulong, ...`; `grep -c 'request: u64' src/backend_term.rs` prints 0.
- **B01-AC-4** — The `TIOCGWINSZ` comment names the architectures it holds for. *Verify:* `grep -n -A1 'TIOCGWINSZ is' src/backend_term.rs`.
- **B01-AC-5** — `make i686` runs the i686 term `cargo test` with 0 failed, replays `tests/fixtures/ref.sav` to `1f22670bf826471a`, and matches all five golden dumps. *Verify:* the CI `i686` job log.
- **B01-AC-6** — The check workflow runs `make i686` as its own job on every pull request, and `make check` neither depends on nor calls it. *Verify:* the workflow file; `make -n check | grep -c i686` prints 0.
- **B01-AC-7** — `CLAUDE.md`'s "Verification" section names `make i686` and what it checks. *Verify:* `grep -n 'make i686' CLAUDE.md` shows a line in "Verification".
- **B01-AC-8** — `make check` is green in CI; goldens, frames, `xhash` and bands are unchanged. *Verify:* the check job's run URL; `git diff --stat <base>..HEAD -- tests/golden 'tests/*-band.json' tests/fixtures` empty.

## Risks
- `script` and `stty` behave differently across util-linux versions; the script must not depend
  on timing beyond a bounded wait.
- i686-musl linking needs a 32-bit C toolchain in CI (the audit used `gcc-multilib`).
- The i686 build proves the 32-bit FFI, not real-i486 support: i686 code needs a P6 CPU. The
  i486 artifact stays the ISO factory's (SPEC.md "Scope").
- Interactive playtest on a real console is an owner step after merge (principle 6).
- Out of scope: the cropped title on small terminals (SPEC.md "Known issues not planned").
