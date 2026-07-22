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

.PHONY: check build test goldens solve sim size build-term test-term frames targets xhash

check: build test test-term goldens frames xhash solve sim size

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
