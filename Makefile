# Makefile — `make check` runs the whole AGENTS.md verification gate in one
# command: warning-free release build (both cargo backends), cargo test
# (both backends), golden-dump diff, frame-golden diff, cross-backend
# replay-hash gate, solver winnability gate, sim-bot balance gate, and
# UPX-packed size budget. POSIX sh recipes (no bashisms); see
# AGENTS.md/CLAUDE.md for the manual workflow this encodes.
#
# The sim-bot balance gate (`sim` target) runs BOTH bot policies — greedy
# (tests/sim-band.json) and pacifist (tests/pacifist-band.json, batch 5 T2,
# DECISION.md item 3) — so `make check` gates mercy's viability alongside
# violence's, not instead of it.
#
# `make targets` is a separate reporting tool: it prints a stripped/packed
# size scoreboard for both backends. It is not part of `check` — `xhash`
# already builds both flavors as the correctness gate.

UPX ?= upx
BUDGET ?= 1474560
SOLVE_SEEDS ?= 10000
SIM_SEEDS ?= 5000

# upx itself reads an environment variable literally named UPX as a source
# of default command-line options (see `upx --help`). make auto-exports
# command-line-set variables into recipe shells, so a bare `UPX=path` on
# the command line would otherwise leak into upx's own environment and be
# misparsed as options ("invalid string ... in environment variable
# 'UPX'"). Keep it a make-only variable.
unexport UPX

BIN := target/release/rl144
GOLDEN_SEEDS := 1 2 3 42 1337

TERM_BIN := target/term/release/rl144
FRAME_SEEDS := 1 42

REF_SAVE := tests/fixtures/ref.sav

.PHONY: check build test goldens solve sim size build-term test-term frames targets xhash msrv

check: build test test-term goldens frames xhash solve sim size msrv

build:
	RUSTFLAGS="-D warnings" cargo build --release

test:
	cargo test --quiet

# Term-flavor build: separate target-dir so it never clobbers the default
# (backend-minifb) build artifacts checked by `build`/`size`.
build-term:
	RUSTFLAGS="-D warnings" cargo build --release --no-default-features --features backend-term --target-dir target/term

test-term:
	cargo test --quiet --no-default-features --features backend-term

# MSRV enforcement (batch 12 T1, "third strike"). Cargo.toml declares
# `rust-version = "1.75"`, but `build`/`test` above only ever exercise
# whatever rustc happens to be on PATH in THIS environment — which is how
# a real E0716 (temporary value dropped while borrowed) in sim_main's
# band-key match got reported and fixed-in-review TWICE and then
# duplicated by batch 10 into two more arms without ever tripping a gate:
# this box's ambient rustc was newer than 1.75 (whose temporary-lifetime-
# extension rules are stricter about borrowing a literal array straight
# out of a match arm), so the risky pattern quietly compiled here every
# time and the failure only ever showed up in human review, not CI.
#
# If `rustup` is on PATH, this target runs the REAL pinned check — `cargo
# +1.75 check` (installing the 1.75 toolchain first if it isn't already)
# for both feature sets — and FAILS the gate on any error, exactly like a
# reintroduced E0716 should. If `rustup` is NOT available (the case in
# this sandboxed environment: no rustup, no `+1.75` toolchain, no apt
# package index/network to fetch one — verified before writing this
# target, see the batch-12 T1 report), it falls back to an ordinary
# `cargo check` under the ambient toolchain and prints a loud warning that
# MSRV was NOT actually verified this run. That fallback is a
# documentation/best-effort device, not real enforcement — it is the
# "at minimum add cargo check under the declared rust-version" floor, not
# the ceiling. CI, which has network access, MUST install a real 1.75
# toolchain and run the pinned check as its own gate step:
#   rustup toolchain install 1.75
#   rustup run 1.75 cargo check --release --features backend-minifb
#   rustup run 1.75 cargo check --release --no-default-features --features backend-term
# Do not treat a green `make check` on a rustup-less box as MSRV-clean;
# treat it as "MSRV unverified, see the warning."
msrv:
	@if command -v rustup >/dev/null 2>&1; then \
		rustup toolchain install 1.75 >/dev/null 2>&1 || true; \
		ok=1; \
		rustup run 1.75 cargo check --release --features backend-minifb || ok=0; \
		rustup run 1.75 cargo check --release --no-default-features --features backend-term || ok=0; \
		if [ "$$ok" -eq 1 ]; then \
			echo "msrv: OK (pinned rustc 1.75, both backends)"; \
		else \
			echo "msrv: FAIL — cargo check under pinned rustc 1.75 failed (both backends must pass)"; \
			exit 1; \
		fi; \
	else \
		echo "warning: rustup not found in this environment — cannot pin rustc 1.75."; \
		echo "warning: falling back to 'cargo check' under the ambient toolchain ($$(rustc --version)), which does NOT verify the declared rust-version = \"1.75\" MSRV from Cargo.toml."; \
		echo "warning: CI MUST install a real 1.75 toolchain and run the pinned check itself — see this target's comment in the Makefile for the exact commands."; \
		cargo check --release --features backend-minifb && \
		cargo check --release --no-default-features --features backend-term; \
	fi

# Regenerate dumps for the golden seeds into a scratch temp dir and cmp them
# against the committed fixtures in tests/golden/. Never writes into the
# working tree; the temp dir is removed on exit, success or failure.
goldens: build
	@tmpdir=`mktemp -d` || exit 1; \
	trap 'rm -rf "$$tmpdir"' EXIT INT TERM HUP; \
	status=0; \
	for seed in $(GOLDEN_SEEDS); do \
		./$(BIN) --dump --seed $$seed > "$$tmpdir/seed_$$seed.txt"; \
		if ! cmp -s "$$tmpdir/seed_$$seed.txt" "tests/golden/seed_$$seed.txt"; then \
			echo "golden mismatch for seed $$seed:"; \
			diff -u "tests/golden/seed_$$seed.txt" "$$tmpdir/seed_$$seed.txt" || true; \
			status=1; \
		fi; \
	done; \
	rm -rf "$$tmpdir"; \
	trap - EXIT INT TERM HUP; \
	if [ "$$status" -ne 0 ]; then \
		echo "goldens: FAIL"; \
		exit 1; \
	fi; \
	echo "goldens: OK ($(GOLDEN_SEEDS))"

# Regenerate --render-frame output for the frame-golden seeds (plus the
# --ascii variant) into a scratch temp dir and cmp them against the
# committed fixtures in tests/golden/. Same never-touch-the-working-tree
# discipline as `goldens`.
frames: build-term
	@tmpdir=`mktemp -d` || exit 1; \
	trap 'rm -rf "$$tmpdir"' EXIT INT TERM HUP; \
	status=0; \
	for seed in $(FRAME_SEEDS); do \
		./$(TERM_BIN) --render-frame --seed $$seed > "$$tmpdir/frame_seed_$$seed.bin"; \
		if ! cmp -s "$$tmpdir/frame_seed_$$seed.bin" "tests/golden/frame_seed_$$seed.bin"; then \
			echo "frame golden mismatch for seed $$seed"; \
			status=1; \
		fi; \
	done; \
	./$(TERM_BIN) --render-frame --seed 1 --ascii > "$$tmpdir/frame_seed_1_ascii.bin"; \
	if ! cmp -s "$$tmpdir/frame_seed_1_ascii.bin" "tests/golden/frame_seed_1_ascii.bin"; then \
		echo "frame golden mismatch for seed 1 (ascii)"; \
		status=1; \
	fi; \
	rm -rf "$$tmpdir"; \
	trap - EXIT INT TERM HUP; \
	if [ "$$status" -ne 0 ]; then \
		echo "frames: FAIL"; \
		exit 1; \
	fi; \
	echo "frames: OK ($(FRAME_SEEDS), +ascii)"

# Cross-backend replay-hash gate: replay the committed fixture save through
# both flavors and require an identical state hash. This is the proof that
# the core/crust seam is real — backend-minifb and backend-term share the
# same deterministic core, so the same seed + input log must converge to
# the same state regardless of which frontend built the binary.
xhash: build build-term
	@h1=`./$(BIN) --replay $(REF_SAVE) | sed -n 's/.*"hash":"\([0-9a-f]*\)".*/\1/p'`; \
	h2=`./$(TERM_BIN) --replay $(REF_SAVE) | sed -n 's/.*"hash":"\([0-9a-f]*\)".*/\1/p'`; \
	if [ -z "$$h1" ] || [ -z "$$h2" ]; then \
		echo "xhash: FAIL — could not extract hash from --replay output"; \
		exit 1; \
	fi; \
	if [ "$$h1" != "$$h2" ]; then \
		echo "xhash: FAIL — minifb=$$h1 term=$$h2"; \
		exit 1; \
	fi; \
	echo "xhash: OK ($$h1)"

solve: build
	./$(BIN) --solve $(SOLVE_SEEDS)

sim: build
	./$(BIN) --sim $(SIM_SEEDS)
	./$(BIN) --sim $(SIM_SEEDS) --policy pacifist
	./$(BIN) --sim $(SIM_SEEDS) --policy tactical
	./$(BIN) --sim $(SIM_SEEDS) --policy tactical-pacifist

# Pack a copy of the release binary (never target/) and enforce the floppy
# budget. If $(UPX) isn't runnable, warn and fall back to reporting the
# stripped size only; that path still succeeds.
size: build
	@stripped=`wc -c < $(BIN) | tr -d ' '`; \
	echo "stripped size: $$stripped bytes"; \
	if ! "$(UPX)" --version >/dev/null 2>&1; then \
		echo "warning: \$$(UPX)='$(UPX)' not runnable; skipping pack/budget check"; \
		exit 0; \
	fi; \
	tmpfile=`mktemp` || exit 1; \
	trap 'rm -f "$$tmpfile"' EXIT INT TERM HUP; \
	cp $(BIN) "$$tmpfile" || exit 1; \
	chmod +x "$$tmpfile" || exit 1; \
	if ! "$(UPX)" --best --lzma -qq "$$tmpfile" >/dev/null 2>&1; then \
		echo "warning: '$(UPX)' failed to pack binary; skipping budget check"; \
		rm -f "$$tmpfile"; \
		trap - EXIT INT TERM HUP; \
		exit 0; \
	fi; \
	packed=`wc -c < "$$tmpfile" | tr -d ' '`; \
	rm -f "$$tmpfile"; \
	trap - EXIT INT TERM HUP; \
	echo "packed size: $$packed bytes (budget $(BUDGET))"; \
	if [ "$$packed" -gt "$(BUDGET)" ]; then \
		echo "size: FAIL — packed $$packed bytes exceeds budget $(BUDGET) bytes"; \
		exit 1; \
	fi; \
	echo "size: OK"

# Size scoreboard across both cargo backends: stripped and UPX-packed bytes
# for minifb and term, plus percent of the floppy budget each consumes.
# Reporting only — not part of `check` (`xhash` already builds and
# exercises both flavors as the correctness gate; this target is for
# watching the number). Still enforces the budget per row, same
# graceful-degradation-if-UPX-is-missing pattern as `size`.
targets: build build-term
	@upx_ok=1; \
	if ! "$(UPX)" --version >/dev/null 2>&1; then \
		echo "warning: \$$(UPX)='$(UPX)' not runnable; packed column will be n/a"; \
		upx_ok=0; \
	fi; \
	status=0; \
	printf '%-8s %12s %12s %6s\n' target stripped packed pct; \
	for pair in "minifb:$(BIN)" "term:$(TERM_BIN)"; do \
		name=$${pair%%:*}; bin=$${pair#*:}; \
		stripped=`wc -c < "$$bin" | tr -d ' '`; \
		packed=""; \
		if [ "$$upx_ok" -eq 1 ]; then \
			tmpfile=`mktemp` || exit 1; \
			trap 'rm -f "$$tmpfile"' EXIT INT TERM HUP; \
			cp "$$bin" "$$tmpfile" || exit 1; \
			chmod +x "$$tmpfile" || exit 1; \
			if "$(UPX)" --best --lzma -qq "$$tmpfile" >/dev/null 2>&1; then \
				packed=`wc -c < "$$tmpfile" | tr -d ' '`; \
			else \
				echo "warning: '$(UPX)' failed to pack $$name binary; skipping its budget check"; \
			fi; \
			rm -f "$$tmpfile"; \
			trap - EXIT INT TERM HUP; \
		fi; \
		if [ -n "$$packed" ]; then \
			pct=$$(( $$packed * 100 / $(BUDGET) )); \
			printf '%-8s %12s %12s %5s%%\n' "$$name" "$$stripped" "$$packed" "$$pct"; \
			if [ "$$packed" -gt "$(BUDGET)" ]; then \
				echo "targets: FAIL — $$name packed $$packed bytes exceeds budget $(BUDGET) bytes"; \
				status=1; \
			fi; \
		else \
			printf '%-8s %12s %12s %6s\n' "$$name" "$$stripped" "n/a" "n/a"; \
		fi; \
	done; \
	if [ "$$status" -ne 0 ]; then \
		exit 1; \
	fi
