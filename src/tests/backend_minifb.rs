use super::*;

/// flash() channel math: 30% brighten per channel, clamped to 255.
/// Pure RGB arithmetic — the visual feel is playtest-only, but the
/// math itself is headlessly checkable.
#[test]
fn flash_channel_math() {
    assert_eq!(flash(0x000000), 0x000000);
    assert_eq!(flash(0x646464), 0x828282); // 100 * 1.3 = 130, no clamp
    assert_eq!(flash(0xC8C8C8), 0xFFFFFF); // 200 * 1.3 = 260 -> clamp 255
    assert_eq!(flash(0xFFFFFF), 0xFFFFFF); // already saturated
}
