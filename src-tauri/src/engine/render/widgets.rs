use super::*;

pub(in crate::engine) const MENU_FOOTER_BOX: UiBox = UiBox {
    x: 39,
    y: 144,
    width: 162,
    height: 16,
};

pub(in crate::engine) fn draw_asset_focus(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    frame.outline(x, y, width, height, AMBER);
    for (corner_x, direction_x) in [(x, 1), (x + width - 1, -1)] {
        for corner_y in [y, y + height - 1] {
            let start_x = if direction_x > 0 {
                corner_x
            } else {
                corner_x - 3
            };
            frame.rect(start_x, corner_y, 4, 2, PARCH);
        }
    }
}

/// How one 5x7 rune is drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::engine) enum RuneStyle {
    /// Outline in the meter color around a parchment core.
    Lit,
    /// An ash outline.
    Unlit,
    /// Earned by volume but held back by a mastery gate: an amber outline
    /// split by a diagonal crack, with no parchment core.
    Cracked,
}

/// The crack across a cracked rune, from its right edge to its left edge.
pub(in crate::engine) const RUNE_CRACK: [(i32, i32); 3] = [(3, 2), (2, 3), (1, 4)];

pub(in crate::engine) fn draw_oracle_rune(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    lit: bool,
    color: Color,
) {
    let style = if lit {
        RuneStyle::Lit
    } else {
        RuneStyle::Unlit
    };
    draw_rune(frame, x, y, style, color);
}

pub(in crate::engine) fn draw_rune(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    style: RuneStyle,
    color: Color,
) {
    let outline = match style {
        RuneStyle::Lit => color,
        RuneStyle::Unlit => ASH,
        RuneStyle::Cracked => AMBER,
    };
    for (dx, dy) in [
        (2, 0),
        (1, 1),
        (3, 1),
        (0, 2),
        (4, 2),
        (0, 3),
        (4, 3),
        (0, 4),
        (4, 4),
        (1, 5),
        (3, 5),
        (2, 6),
    ] {
        frame.pixel(x + dx, y + dy, outline);
    }
    match style {
        RuneStyle::Lit => {
            for (dx, dy) in [(2, 2), (1, 3), (2, 3), (3, 3), (2, 4)] {
                frame.pixel(x + dx, y + dy, PARCH);
            }
        }
        RuneStyle::Cracked => {
            for (dx, dy) in RUNE_CRACK {
                frame.pixel(x + dx, y + dy, AMBER);
            }
        }
        RuneStyle::Unlit => {}
    }
}

/// Width of a three-rune meter: three 5px runes at a 7px pitch.
pub(in crate::engine) const RUNE_METER_WIDTH: i32 = 19;

pub(in crate::engine) fn draw_oracle_rune_meter(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    lit_runes: usize,
    color: Color,
) {
    draw_mastery_meter(frame, x, y, lit_runes, 0, color);
}

/// Three runes: `lit` in `color`, then `cracks` cracked ones (earned by
/// volume, held back by a mastery gate), then unlit ones.
pub(in crate::engine) fn draw_mastery_meter(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    lit: usize,
    cracks: usize,
    color: Color,
) {
    draw_rune_row(frame, x, y, mastery_meter_styles(lit, cracks), color);
}

/// How each of a meter's three runes is drawn for `lit` lit runes followed
/// by `cracks` cracked ones.
pub(in crate::engine) fn mastery_meter_styles(lit: usize, cracks: usize) -> [RuneStyle; 3] {
    std::array::from_fn(|index| {
        if index < lit {
            RuneStyle::Lit
        } else if index < lit + cracks {
            RuneStyle::Cracked
        } else {
            RuneStyle::Unlit
        }
    })
}

pub(in crate::engine) fn draw_rune_row(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    styles: [RuneStyle; 3],
    color: Color,
) {
    for (index, style) in styles.into_iter().enumerate() {
        draw_rune(frame, x + index as i32 * 7, y, style, color);
    }
}

pub(in crate::engine) fn ward_color(hearts: u8) -> Color {
    match hearts {
        3.. => CYAN,
        2 => AMBER,
        _ => RED,
    }
}

pub(in crate::engine) fn draw_oracle_ward_meter(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    hearts: u8,
) {
    let color = ward_color(hearts);
    frame.text(x, y, "WARD", color, 1);
    draw_oracle_rune_meter(frame, x + 27, y, hearts.min(3) as usize, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_ward_runes_show_each_health_threshold_without_generic_pips() {
        let mut full = Framebuffer::default();
        let mut strained = Framebuffer::default();
        let mut fractured = Framebuffer::default();
        let mut broken = Framebuffer::default();
        draw_oracle_ward_meter(&mut full, 0, 0, 3);
        draw_oracle_ward_meter(&mut strained, 0, 0, 2);
        draw_oracle_ward_meter(&mut fractured, 0, 0, 1);
        draw_oracle_ward_meter(&mut broken, 0, 0, 0);

        assert_ne!(full.pixels, strained.pixels);
        assert_ne!(strained.pixels, fractured.pixels);
        assert_ne!(fractured.pixels, broken.pixels);
        assert!(color_pixels_in_region(&full.pixels, CYAN, 0..46, 0..7) > 0);
        assert!(color_pixels_in_region(&strained.pixels, AMBER, 0..46, 0..7) > 0);
        assert!(color_pixels_in_region(&fractured.pixels, RED, 0..46, 0..7) > 0);
    }
}
