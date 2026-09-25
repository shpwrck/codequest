use super::*;

pub(super) const HERO_SPRITE_WIDTH: usize = 24;
pub(super) const HERO_SPRITE_HEIGHT: usize = 36;
pub(super) const HERO_SPRITE_BYTES: usize = HERO_SPRITE_WIDTH * HERO_SPRITE_HEIGHT * 4;
pub(super) const HERO_PORTRAIT_SIZE: usize = 24;
pub(super) const HERO_PORTRAIT_BYTES: usize = HERO_PORTRAIT_SIZE * HERO_PORTRAIT_SIZE * 4;
pub(super) const DROP_SPRITE_SIZE: usize = 16;
pub(super) const DROP_SPRITE_BYTES: usize = DROP_SPRITE_SIZE * DROP_SPRITE_SIZE * 4;

pub(super) const ORACLE_CHRONICLE: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/chronicle.rgb");
pub(super) const ORACLE_AWAKENING_SOURCE: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/awakening-source.rgb");
pub(super) const ORACLE_AWAKENING_SIGNAL: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/awakening-signal.rgb");
pub(super) const ORACLE_AWAKENING_ARCHIVE: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/awakening-archive.rgb");
pub(super) const ORACLE_AWAKENING_CONVERGENCE: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/awakening-convergence.rgb");
pub(super) const ORACLE_AWAKENING: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/awakening.rgb");
pub(super) const ORACLE_GATEWAY: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/gateway.rgb");
pub(super) const ORACLE_ATELIER: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/atelier.rgb");
pub(super) const ORACLE_SANCTUM: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/sanctum.rgb");
pub(super) const ORACLE_TRIAL: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/trial.rgb");
pub(super) const ORACLE_ASCENSION: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/ascension.rgb");
pub(super) const ORACLE_AFTERMATH: &[u8; NATIVE_RGB_BYTES] =
    include_bytes!("../../assets/oracle/aftermath.rgb");

pub(super) const ORACLE_HEROES: [&[u8; HERO_SPRITE_BYTES]; 5] = [
    include_bytes!("../../assets/oracle/hero-magenta.rgba"),
    include_bytes!("../../assets/oracle/hero-cyan.rgba"),
    include_bytes!("../../assets/oracle/hero-emerald.rgba"),
    include_bytes!("../../assets/oracle/hero-amber.rgba"),
    include_bytes!("../../assets/oracle/hero-violet.rgba"),
];

pub(super) const ORACLE_PORTRAITS: [&[u8; HERO_PORTRAIT_BYTES]; 5] = [
    include_bytes!("../../assets/oracle/portrait-magenta.rgba"),
    include_bytes!("../../assets/oracle/portrait-cyan.rgba"),
    include_bytes!("../../assets/oracle/portrait-emerald.rgba"),
    include_bytes!("../../assets/oracle/portrait-amber.rgba"),
    include_bytes!("../../assets/oracle/portrait-violet.rgba"),
];

pub(super) const ORACLE_DATA_DROP: &[u8; DROP_SPRITE_BYTES] =
    include_bytes!("../../assets/oracle/drop-data.rgba");
pub(super) const ORACLE_BUG_DROP: &[u8; DROP_SPRITE_BYTES] =
    include_bytes!("../../assets/oracle/drop-bug.rgba");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_drop_sprites_are_authored_distinct_and_contained() {
        assert_ne!(ORACLE_DATA_DROP, ORACLE_BUG_DROP);

        for (name, sprite) in [
            ("data", ORACLE_DATA_DROP.as_slice()),
            ("bug", ORACLE_BUG_DROP.as_slice()),
        ] {
            let visible_pixels = sprite
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel[3] > 0)
                .count();
            let bright_pixels = sprite
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| {
                    pixel[3] > 0 && pixel[..3].iter().copied().max().unwrap_or(0) >= 128
                })
                .count();
            assert!(
                visible_pixels >= 45,
                "the {name} sprite needs a readable authored silhouette"
            );
            assert!(
                bright_pixels >= 20,
                "the {name} sprite needs a high-contrast luminous core"
            );
        }

        let playfield = LayoutBounds {
            x: 0,
            y: 15,
            width: WIDTH as i32,
            height: 128,
        };
        let offset = DROP_SPRITE_SIZE as i32 / 2;
        for x in [24, 54, 82, 112, 142, 210] {
            for y in [30, 127] {
                let drop = LayoutBounds {
                    x: x - offset,
                    y: y - offset,
                    width: DROP_SPRITE_SIZE as i32,
                    height: DROP_SPRITE_SIZE as i32,
                };
                assert!(
                    bounds_contains(playfield, drop),
                    "drop at ({x}, {y}) exceeds the sanctum playfield"
                );
            }
        }
    }
}
