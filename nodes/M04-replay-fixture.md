# M04 — replay-fixture

| Track | Deps | Unlocks | Size |
|---|---|---|---|
| done | S01 | B02 | S |

**Owns:** `tests/fixtures/`, the reference save `ref.sav` that the cross-backend replay gate replays.

## Purpose
`tests/fixtures/ref.sav` is a 19-byte save: the `RL14` header, version byte 5, the seed 4242 as a little-endian
`u64`, and the input log `1 3 3 3 4 1` (south, then east three times onto the overworld's hole `V`, which drops
the player into the root dungeon, then a wait and a step south there). It holds no restart or retry byte. `make xhash` replays it
through both backends' `--replay` and requires the same state hash from each; today both print
`{"hash":"1f22670bf826471a","seed":4242,"turns":6,"depth":1,"hp":20,"light":1998,"dead":false,"won":false}`.
Its only job is to be a save both builds parse and replay; it was regenerated when the front door moved to the
overworld (commit 7e1fe44).

## Tests
`make xhash` (target of `Makefile`, which prints `xhash: FAIL` on a missing or differing hash). Unit tests in
`src/main.rs` such as `v1_save_replays_under_v7_parsing` cover old save versions with their own byte blobs,
not with this file.

## Changes
none: imported

## Gate
- **M04-AC-1** — `ref.sav` parses as a save in both builds, and replaying it gives both the same state hash. *Verify:* `make xhash` (target of `Makefile`).
- **M04-AC-2** — `ref.sav` is a version-5 save of seed 4242 with the six inputs `1 3 3 3 4 1`, whose replay crosses into the root dungeon and ends alive at depth 1 with hash `1f22670bf826471a`. *Verify:* unpinned (`make xhash` compares the two builds with each other, not with a recorded hash or state).

## Risks
- No test records the expected hash, so a change to the hashed state shape or to replay moves the hash without
  failing anything as long as both builds move together; `docs/design/486/2026-08-04-boot-on-a-486-scope.md`
  quotes `1f22670bf826471a` as the value to expect across architectures.
- The fixture is a version-5 save while `SAVE_VERSION` is 11: it relies on `parse_save` accepting every older
  version (`CLAUDE.md`'s save-version doctrine). Its short path never reaches the objective and never talks,
  gives or uses, so most hashed fields stay at their defaults.
- `v1_save_replays_under_v7_parsing`'s doc comment calls this file "exactly such a v1 blob, containing a byte
  5"; the file is a version-5 blob with no byte 5, so that comment is stale.
- `.noodlebot/roles.yaml` makes `tests/fixtures/**` a tester path, so only the tester may change this file.
