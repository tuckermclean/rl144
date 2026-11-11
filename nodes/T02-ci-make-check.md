# T02 — ci-make-check

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| now | none | B01, B02, D01, E01, H01, S01, T01 | S |

**Owns:** `.github/workflows/check.yml`, `CLAUDE.md` (the "Verification" section's CI sentence)
**Decisions:** OD2

## Purpose
The repository has never run CI: there is no `.github/`, and the only gate results are in commit
messages (audit #15, §3 "CI comparison"). Every later node needs a CI run as evidence for its
build criteria, so this node comes first. The audit ran `make check` green at `f33a80e` in
`rust:1.75-slim` after installing `make`, `g++` (minifb compiles a C++ shim) and upx 4.2.4 from
the upstream release (Debian 12 has no `upx-ucl`); it took about 36 minutes.

## Tests
No Rust tests. The evidence is the workflow's own runs. Red target: the workflow file is absent
at the base, so `test -f .github/workflows/check.yml` exits 1 there and 0 after; the real check
is a green run of the workflow on this node's PR and a red run on a deliberately broken commit.

## Changes
1. Add `.github/workflows/check.yml`: triggers `pull_request`, `push` to `main` and
   `workflow_dispatch`; one job in the `rust:1.75-slim` container. *T02-AC-1.*
2. Install `make`, `g++`, `xz-utils` with apt, and upx 4.2.4 from the upstream release tarball,
   checking its SHA-256 before use. *T02-AC-2.*
3. Run `make check` as the job's step, with a 120-minute job timeout. *T02-AC-1, T02-AC-3.*
4. Prove a failing gate fails the job with a temporary commit on the node branch that lowers
   the Makefile's `BUDGET` to 1000, then revert it before the PR is ready. *T02-AC-4.*
5. In `CLAUDE.md`'s "Verification" section, say that `.github/workflows/check.yml` runs
   `make check` on every pull request (the "Building a node" section already names it).
   *T02-AC-5.*

## Gate
- **T02-AC-1** — `.github/workflows/check.yml` runs `make check` in the `rust:1.75-slim` container on `pull_request`, on `push` to `main` and on `workflow_dispatch`, and its run on this node's PR head is green. *Verify:* the workflow file, and the PR head's run URL with conclusion `success`.
- **T02-AC-2** — The job installs `make`, `g++`, `xz-utils` and upx 4.2.4 from the upstream release with its SHA-256 checked, and nothing else beyond the base image. *Verify:* the workflow file and the run log's install step.
- **T02-AC-3** — The run's log shows every gate line of `make check`, including `goldens: OK (1 2 3 42 1337)`, `frames: OK`, `xhash: OK (1f22670bf826471a)`, `size: OK` and `msrv: OK (pinned rustc 1.75, both backends)`, and no "not runnable" or "MSRV was NOT actually verified" warning. *Verify:* `grep` of the downloaded run log for each line.
- **T02-AC-4** — A commit that makes one gate fail makes the job fail, and the commit is reverted before merge. *Verify:* the red run's URL (failing at `size: FAIL`), and `git diff <base>..HEAD -- Makefile` empty at the PR head.
- **T02-AC-5** — `CLAUDE.md`'s "Verification" section names `.github/workflows/check.yml` as the CI that runs `make check`, and the "Building a node" section is unchanged. *Verify:* `grep -n 'check.yml' CLAUDE.md` shows a line in "Verification"; `git diff <base>..HEAD -- CLAUDE.md` has no hunk in "Building a node".

## Risks
- Pushing a change under `.github/workflows/` needs a credential with workflow permission; if
  the run's credential lacks it, the owner pushes the branch (node skill step 6).
- A run takes about 36 minutes; `make test` alone is about 13. Caching `~/.cargo` and `target/`
  is allowed if it does not change any gate's result.
- Packed sizes depend on the upx version; pinning 4.2.4 keeps CI numbers comparable with the
  audit's.
- The temporary failing commit must touch only the Makefile's `BUDGET`, never a golden or band
  file (SPEC.md principle 7).
- Out of scope: rustfmt or clippy gates, the i686 job (B01), release builds (S01).
