# Makefile — `make check` runs the whole AGENTS.md verification gate in one
# command: warning-free release build, cargo test, golden-dump diff, solver
# winnability gate, and UPX-packed size budget. POSIX sh recipes (no
# bashisms); see AGENTS.md/CLAUDE.md for the manual workflow this encodes.

UPX ?= upx
BUDGET ?= 1474560
SOLVE_SEEDS ?= 10000

# upx itself reads an environment variable literally named UPX as a source
# of default command-line options (see `upx --help`). make auto-exports
# command-line-set variables into recipe shells, so a bare `UPX=path` on
# the command line would otherwise leak into upx's own environment and be
# misparsed as options ("invalid string ... in environment variable
# 'UPX'"). Keep it a make-only variable.
unexport UPX

BIN := target/release/rl144
GOLDEN_SEEDS := 1 2 3 42 1337

.PHONY: check build test goldens solve size

check: build test goldens solve size

build:
	RUSTFLAGS="-D warnings" cargo build --release

test:
	cargo test --quiet

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

solve: build
	./$(BIN) --solve $(SOLVE_SEEDS)

# Pack a copy of the release binary (never target/) and enforce the floppy
# budget. If $(UPX) isn't runnable, warn and fall back to reporting the
# stripped size only; that path still succeeds.
size: build
	@stripped=`wc -c < $(BIN) | tr -d ' '`; \
	echo "stripped size: $$stripped bytes"; \
	if ! $(UPX) --version >/dev/null 2>&1; then \
		echo "warning: \$$(UPX)='$(UPX)' not runnable; skipping pack/budget check"; \
		exit 0; \
	fi; \
	tmpfile=`mktemp` || exit 1; \
	trap 'rm -f "$$tmpfile"' EXIT INT TERM HUP; \
	cp $(BIN) "$$tmpfile"; \
	chmod +x "$$tmpfile"; \
	if ! $(UPX) --best --lzma -qq "$$tmpfile" >/dev/null 2>&1; then \
		echo "warning: $(UPX) failed to pack binary; skipping budget check"; \
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
