use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct UiBox {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

#[derive(Resource)]
pub(super) struct Framebuffer {
    pub(super) pixels: Vec<u8>,
}

/// A plate's address and grade (base, cyan, and gold scales), or no grade.
pub(super) type PlateKey = (usize, Option<[u16; 3]>);

thread_local! {
    /// RGBA expansions of the RGB plates drawn on this thread.
    static PLATES: RefCell<Vec<(PlateKey, Box<[u8]>)>> = const { RefCell::new(Vec::new()) };
}

/// Expands an RGB plate to opaque RGBA. A grade scales cyan-leaning pixels,
/// gold-leaning pixels, and all others by its three `/255` factors.
pub(super) fn expand_plate(rgb: &[u8; NATIVE_RGB_BYTES], grade: Option<[u16; 3]>, rgba: &mut [u8]) {
    let pixels = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .zip(rgba.as_chunks_mut::<4>().0.iter_mut());
    let Some([base, cyan, gold]) = grade else {
        for (source, destination) in pixels {
            destination.copy_from_slice(&[source[0], source[1], source[2], 255]);
        }
        return;
    };
    for (source, destination) in pixels {
        let [red, green, blue] = [source[0] as u16, source[1] as u16, source[2] as u16];
        let scale = if is_cyan_pixel(red, green, blue) {
            cyan
        } else if is_gold_pixel(red, green, blue) {
            gold
        } else {
            base
        };
        destination.copy_from_slice(&[
            (red * scale / 255) as u8,
            (green * scale / 255) as u8,
            (blue * scale / 255) as u8,
            255,
        ]);
    }
}

pub(super) fn is_cyan_pixel(red: u16, green: u16, blue: u16) -> bool {
    blue > red.saturating_add(12) && green > red
}

pub(super) fn is_gold_pixel(red: u16, green: u16, blue: u16) -> bool {
    red > blue.saturating_add(14) && green > blue
}

/// Which awakening strength lights each pixel of the awakening plate: 0 the
/// Oracle's center diamond, 1 cyan, 2 gold, 3 ambient. The plate is constant,
/// so the classes are computed once.
pub(super) fn awakening_classes() -> &'static [u8] {
    static CLASSES: OnceLock<Box<[u8]>> = OnceLock::new();
    CLASSES.get_or_init(|| {
        ORACLE_AWAKENING
            .as_chunks::<3>()
            .0
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let x = (index % WIDTH) as i32;
                let y = (index / WIDTH) as i32;
                let [red, green, blue] = [source[0] as u16, source[1] as u16, source[2] as u16];
                if (x - 120).abs() + (y - 80).abs() < 47 {
                    0
                } else if is_cyan_pixel(red, green, blue) {
                    1
                } else if is_gold_pixel(red, green, blue) {
                    2
                } else {
                    3
                }
            })
            .collect()
    })
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self {
            pixels: vec![0; FRAME_BYTES],
        }
    }
}

impl Framebuffer {
    pub(super) fn clear(&mut self, color: Color) {
        for pixel in self.pixels.as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&[color.0, color.1, color.2, 255]);
        }
    }

    pub(super) fn blit_rgb(&mut self, rgb: &'static [u8; NATIVE_RGB_BYTES]) {
        self.blit_plate(rgb, None);
    }

    pub(super) fn blit_rgb_graded(
        &mut self,
        rgb: &'static [u8; NATIVE_RGB_BYTES],
        base: u16,
        cyan: u16,
        gold: u16,
    ) {
        self.blit_plate(rgb, Some([base, cyan, gold]));
    }

    /// Copies a plate's RGBA expansion, built once per plate and grade on
    /// this thread: every plate and grade is a constant, so expanding it
    /// again each tick would only redo identical work.
    pub(super) fn blit_plate(
        &mut self,
        rgb: &'static [u8; NATIVE_RGB_BYTES],
        grade: Option<[u16; 3]>,
    ) {
        // A `'static` plate's address identifies it for the life of the
        // process; a duplicated constant only costs one more cache entry.
        let key = (rgb.as_ptr() as usize, grade);
        PLATES.with_borrow_mut(|plates| {
            let index = plates
                .iter()
                .position(|(cached, _)| *cached == key)
                .unwrap_or_else(|| {
                    let mut expanded = vec![0; FRAME_BYTES].into_boxed_slice();
                    expand_plate(rgb, grade, &mut expanded);
                    plates.push((key, expanded));
                    plates.len() - 1
                });
            self.pixels.copy_from_slice(&plates[index].1);
        });
    }

    pub(super) fn blit_awakening(&mut self, ticks: u64) {
        let cyan_strength = 38 + ticks.saturating_sub(36).min(108) as u16 * 190 / 108;
        let gold_strength = 38 + ticks.saturating_sub(112).min(108) as u16 * 197 / 108;
        let center_strength = 38 + ticks.saturating_sub(188).min(48) as u16 * 217 / 48;
        let ambient_strength = 34 + ticks.min(236) as u16 * 38 / 236;
        // One scaled channel table per pixel class, indexed like
        // `AwakeningClass`; each entry is the per-pixel `value * strength / 255`.
        let tables = [
            center_strength,
            cyan_strength,
            gold_strength,
            ambient_strength,
        ]
        .map(|strength| {
            std::array::from_fn::<u8, 256, _>(|value| (value as u16 * strength / 255) as u8)
        });

        for ((source, class), destination) in ORACLE_AWAKENING
            .as_chunks::<3>()
            .0
            .iter()
            .zip(awakening_classes())
            .zip(self.pixels.as_chunks_mut::<4>().0.iter_mut())
        {
            let table = &tables[*class as usize];
            *destination = [
                table[source[0] as usize],
                table[source[1] as usize],
                table[source[2] as usize],
                255,
            ];
        }
    }

    pub(super) fn blit_rgba(
        &mut self,
        rgba: &[u8],
        source_width: usize,
        source_height: usize,
        x: i32,
        y: i32,
        scale: i32,
    ) {
        debug_assert_eq!(rgba.len(), source_width * source_height * 4);
        for source_y in 0..source_height {
            for source_x in 0..source_width {
                let source_index = (source_y * source_width + source_x) * 4;
                let alpha = rgba[source_index + 3] as u16;
                if alpha == 0 {
                    continue;
                }
                for offset_y in 0..scale {
                    for offset_x in 0..scale {
                        let target_x = x + source_x as i32 * scale + offset_x;
                        let target_y = y + source_y as i32 * scale + offset_y;
                        if target_x < 0
                            || target_y < 0
                            || target_x >= WIDTH as i32
                            || target_y >= HEIGHT as i32
                        {
                            continue;
                        }
                        let destination_index = (target_y as usize * WIDTH + target_x as usize) * 4;
                        let inverse = 255 - alpha;
                        for channel in 0..3 {
                            let source_channel = rgba[source_index + channel] as u16;
                            let destination_channel =
                                self.pixels[destination_index + channel] as u16;
                            self.pixels[destination_index + channel] =
                                ((source_channel * alpha + destination_channel * inverse) / 255)
                                    as u8;
                        }
                        self.pixels[destination_index + 3] = 255;
                    }
                }
            }
        }
    }

    pub(super) fn pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
            return;
        }
        let index = (y as usize * WIDTH + x as usize) * 4;
        self.pixels[index..index + 4].copy_from_slice(&[color.0, color.1, color.2, 255]);
    }

    pub(super) fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        for py in y.max(0)..(y + height).min(HEIGHT as i32) {
            for px in x.max(0)..(x + width).min(WIDTH as i32) {
                self.pixel(px, py, color);
            }
        }
    }

    pub(super) fn outline(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        self.rect(x, y, width, 1, color);
        self.rect(x, y + height - 1, width, 1, color);
        self.rect(x, y, 1, height, color);
        self.rect(x + width - 1, y, 1, height, color);
    }

    pub(super) fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = (x1 - x0).abs();
        let step_x = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let step_y = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.pixel(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let doubled = error * 2;
            if doubled >= dy {
                error += dy;
                x0 += step_x;
            }
            if doubled <= dx {
                error += dx;
                y0 += step_y;
            }
        }
    }

    pub(super) fn text(&mut self, x: i32, y: i32, text: &str, color: Color, scale: i32) {
        let mut cursor = x;
        for ch in text.chars().map(|ch| ch.to_ascii_uppercase()) {
            for (gy, row) in glyph(ch).iter().enumerate() {
                for gx in 0..GLYPH_WIDTH {
                    let mask = 1u8 << (GLYPH_WIDTH - 1 - gx) as u32;
                    if row & mask != 0 {
                        self.rect(
                            cursor + gx * scale,
                            y + gy as i32 * scale,
                            scale,
                            scale,
                            color,
                        );
                    }
                }
            }
            cursor += GLYPH_ADVANCE * scale;
        }
    }

    pub(super) fn compact_text(&mut self, x: i32, y: i32, text: &str, color: Color) {
        let mut cursor = x;
        for ch in text.chars().map(|ch| ch.to_ascii_uppercase()) {
            for (glyph_y, row) in glyph(ch).iter().enumerate() {
                for glyph_x in 0..GLYPH_WIDTH {
                    let mask = 1u8 << (GLYPH_WIDTH - 1 - glyph_x) as u32;
                    if row & mask != 0 {
                        self.pixel(cursor + glyph_x, y + glyph_y as i32, color);
                    }
                }
            }
            cursor += GLYPH_WIDTH;
        }
    }

    pub(super) fn centered_text(&mut self, y: i32, text: &str, color: Color, scale: i32) {
        let width = text_width(text, scale);
        self.text((WIDTH as i32 - width) / 2, y, text, color, scale);
    }

    pub(super) fn centered_text_in(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        text: &str,
        color: Color,
        scale: i32,
    ) {
        let rendered_width = text_width(text, scale);
        debug_assert!(
            rendered_width <= width,
            "text `{text}` is wider than its {width}px container"
        );
        self.text(x + (width - rendered_width) / 2, y, text, color, scale);
    }

    pub(super) fn centered_text_box(
        &mut self,
        bounds: UiBox,
        text: &str,
        color: Color,
        scale: i32,
    ) {
        let rendered_width = text_width(text, scale);
        let rendered_height = 7 * scale;
        debug_assert!(
            rendered_width <= bounds.width && rendered_height <= bounds.height,
            "text `{text}` does not fit {bounds:?}"
        );
        self.text(
            bounds.x + (bounds.width - rendered_width) / 2,
            bounds.y + (bounds.height - rendered_height) / 2,
            text,
            color,
            scale,
        );
    }

    pub(super) fn centered_compact_text_box(&mut self, bounds: UiBox, text: &str, color: Color) {
        let rendered_width = text.chars().count() as i32 * GLYPH_WIDTH;
        debug_assert!(
            rendered_width <= bounds.width && 7 <= bounds.height,
            "compact text `{text}` does not fit {bounds:?}"
        );
        self.compact_text(
            bounds.x + (bounds.width - rendered_width) / 2,
            bounds.y + (bounds.height - 7) / 2,
            text,
            color,
        );
    }

    pub(super) fn centered_compact_lines_box(
        &mut self,
        bounds: UiBox,
        lines: &[String],
        color: Color,
    ) {
        let capacity = ((bounds.height + 1) / LINE_HEIGHT) as usize;
        let visible = &lines[..lines.len().min(capacity)];
        let rendered_height = visible.len() as i32 * LINE_HEIGHT - 1;
        let start_y = bounds.y + (bounds.height - rendered_height) / 2;
        for (index, line) in visible.iter().enumerate() {
            self.centered_compact_text_box(
                UiBox {
                    y: start_y + index as i32 * LINE_HEIGHT,
                    height: 7,
                    ..bounds
                },
                line,
                color,
            );
        }
    }

    pub(super) fn wrapped_text(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        max_chars: usize,
        max_lines: usize,
    ) {
        for (line_no, line) in wrap_text(text, max_chars)
            .into_iter()
            .take(max_lines)
            .enumerate()
        {
            self.text(x, y + line_no as i32 * LINE_HEIGHT, &line, color, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framebuffer_is_always_fixed_resolution() {
        let mut engine = GameEngine::new();
        assert_eq!((WIDTH, HEIGHT), (240, 160));
        assert_eq!(FRAME_BYTES, 153_600);
        assert_eq!(engine.frame().len(), FRAME_BYTES);
        assert!(engine
            .frame()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255));
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        for button in [
            Button::Up,
            Button::Down,
            Button::Left,
            Button::Right,
            Button::A,
            Button::B,
            Button::Start,
            Button::Select,
            Button::L,
            Button::R,
        ] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            assert_eq!(
                engine.frame().len(),
                FRAME_BYTES,
                "{button:?} changed the framebuffer size"
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
            assert_eq!(
                engine.frame().len(),
                FRAME_BYTES,
                "releasing {button:?} changed the framebuffer size"
            );
        }
    }

    #[test]
    fn oracle_gameplay_cannot_change_resolution() {
        let mut engine = waiting_oracle_engine();
        for button in [Button::Left, Button::Right, Button::Up, Button::Down] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            for _ in 0..30 {
                engine.update();
                assert_eq!(engine.frame().len(), FRAME_BYTES);
            }
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
    }
}
