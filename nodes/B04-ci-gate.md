# B04 — ci-gate

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | B02 | M05 | S |

**Owns:** `.github/`, the CI workflow that runs the gate.

## Purpose
The repository has no CI: the gate (`PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.75 make -k check`,
about 35 minutes) runs only by hand, and a node's PR records a local run as its evidence. The owner put this
first (answer Q2 on the adoption's issue #3): the pinned toolchain is already in the gate command, and this
node adds a CI workflow that runs that command on every pull request to `main` and every push to `main`.
The runner must have what the Makefile's checks need to check anything (B02's Risks): rustup with the 1.75
toolchain, so `make msrv` and the pinned gate run 1.75 rather than falling back to the ambient rustc, and
`upx`, so `make size` packs the binary instead of skipping the budget check with a warning.

## Tests
No unit test: the workflow is checked by its CI runs, recorded in the node's PR (a recorded run, as
`CLAUDE.md`'s CI evidence asks). The existing gate is unchanged and must stay green on the runner.

## Changes
1. A workflow under `.github/workflows/` that runs on `pull_request` (base `main`) and on `push` to `main`, on
   a Linux runner, and runs the graph's gate command as one step, so that the step fails when `make` exits
   non-zero.
2. The runner's setup, before the gate step: rustup's `1.75` toolchain installed (the gate command selects
   it through `RUSTUP_TOOLCHAIN`) and `upx` installed (`upx-ucl` or `upx` from the distribution).
3. A job timeout that leaves room for the gate's measured run time (34m43s on the owner's machine).
4. Only when the runner needs it: a change to `Makefile` (B02) that the CI run shows is needed; none is
   expected.

## Gate
- **B04-AC-1** — A workflow under `.github/workflows/` runs the gate command `PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN=1.75 make -k check` on every pull request to `main` and every push to `main`, and its run on the node PR's head commit passes. *Verify:* the CI run on the PR's head commit, linked in the PR, with its log.
- **B04-AC-2** — On the runner, `make size` packs the binary: the run's log has a `packed size: N bytes (budget 1474560)` line and `size: OK`, and no missing-`upx` warning. *Verify:* the same CI run's log, quoted in the PR.
- **B04-AC-3** — On the runner, `make msrv` and the tests run under rustup's 1.75: the log has `msrv: OK` for both feature sets and no fall-back-to-the-ambient-toolchain warning, and `rustc --version` printed by the job reads 1.75. *Verify:* the same CI run's log, quoted in the PR.
- **B04-AC-4** — A failing gate fails the run: a run on a throwaway branch whose single commit on top of the node's head changes one byte of `tests/golden/seed_1.txt` ends as failed, and its log has a `golden mismatch for seed 1` line that `gate.fail_pattern` matches once the log's timestamp prefix and colour escape codes are stripped. The throwaway branch is deleted afterwards and never merged. *Verify:* that CI run, linked in the PR, with the matching log line quoted.
- **B04-AC-5** — The job's timeout is longer than the gate's run time: it is set to at least 60 minutes, and the PR records the passing run's wall time. *Verify:* the workflow file and the run of B04-AC-1.
- **B04-AC-6** — The node changes no source, test, golden or band file: its diff touches only `.github/` (and `Makefile` only if Changes item 4 was needed, with the reason in the PR), and the stripped and packed sizes are unchanged. *Verify:* the PR's diff and the sizes recorded from the CI run's `make size` output.

## Risks
- The gate takes about 35 minutes per run, so every PR and every push to `main` costs that much runner time;
  on a private repository that is billed minutes.
- `minifb` builds C code through `cc`, so the runner needs a C compiler (the standard Linux runners have
  one); its `x11` feature loads X11 at run time, which the headless gate never does.
- `make size` and `make msrv` pass without checking when `upx` or rustup is missing (B02's Risks); B04-AC-2
  and B04-AC-3 exist to show the runner is not in that state, and a later change to the runner image can
  silently put it back there.
- CI's log adds a timestamp prefix to each line (and possibly colour codes); `gate.fail_pattern` matches the
  lines once those are stripped, as `CLAUDE.md`'s "Building a node" section records.
- The size budget check covers only the window binary (B02's Risks); this node does not widen it.
