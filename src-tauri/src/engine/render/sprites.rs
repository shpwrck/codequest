use super::*;

pub(in crate::engine) const HERO_STYLE_COLORS: [Color; 5] = [RED, SKY, GREEN, GOLD, PLUM];

pub(in crate::engine) fn draw_code_sigil(frame: &mut Framebuffer, x: i32, y: i32, mirrored: bool) {
    let edge = if mirrored { -1 } else { 1 };
    frame.line(x, y - 16, x + edge * 12, y, SKY);
    frame.line(x + edge * 12, y, x, y + 16, SKY);
    frame.line(x + edge * 5, y - 16, x + edge * 17, y, ROYAL);
    frame.line(x + edge * 17, y, x + edge * 5, y + 16, ROYAL);
    frame.rect(x + edge.min(0) * 18, y - 2, 18, 4, GOLD);
}

pub(in crate::engine) fn draw_oracle_sigil(
    frame: &mut Framebuffer,
    center_x: i32,
    center_y: i32,
    pulse: i32,
) {
    let radius = 24 + pulse;
    frame.line(center_x - radius, center_y, center_x, center_y - 13, SKY);
    frame.line(center_x, center_y - 13, center_x + radius, center_y, SKY);
    frame.line(center_x + radius, center_y, center_x, center_y + 13, ROYAL);
    frame.line(center_x, center_y + 13, center_x - radius, center_y, ROYAL);
    frame.outline(center_x - 7, center_y - 7, 15, 15, GOLD);
    frame.rect(center_x - 2, center_y - 2, 5, 5, PARCH);
}

pub(in crate::engine) fn draw_commit_constellation(frame: &mut Framebuffer, ticks: u64) {
    let nodes = [
        (36, 98),
        (70, 72),
        (106, 91),
        (142, 62),
        (178, 80),
        (208, 48),
    ];
    for pair in nodes.windows(2) {
        frame.line(pair[0].0, pair[0].1, pair[1].0, pair[1].1, PLUM);
    }
    for (index, (x, y)) in nodes.into_iter().enumerate() {
        let color = if ((ticks / 10) as usize + index).is_multiple_of(3) {
            GOLD
        } else {
            SKY
        };
        frame.rect(x - 2, y - 2, 5, 5, color);
    }
}

pub(in crate::engine) fn draw_oracle_data(frame: &mut Framebuffer, x: i32, y: i32) {
    let offset = DROP_SPRITE_SIZE as i32 / 2;
    frame.blit_rgba(
        ORACLE_DATA_DROP,
        DROP_SPRITE_SIZE,
        DROP_SPRITE_SIZE,
        x - offset,
        y - offset,
        1,
    );
}

pub(in crate::engine) fn draw_oracle_bug(frame: &mut Framebuffer, x: i32, y: i32) {
    let offset = DROP_SPRITE_SIZE as i32 / 2;
    frame.blit_rgba(
        ORACLE_BUG_DROP,
        DROP_SPRITE_SIZE,
        DROP_SPRITE_SIZE,
        x - offset,
        y - offset,
        1,
    );
}

pub(in crate::engine) fn draw_crab(frame: &mut Framebuffer, x: i32, y: i32, scale: i32) {
    frame.rect(x + 4 * scale, y, 20 * scale, 10 * scale, CRAB);
    frame.rect(x, y + 5 * scale, 28 * scale, 8 * scale, CRAB);
    frame.rect(x + 3 * scale, y + 13 * scale, 5 * scale, 4 * scale, CRAB);
    frame.rect(x + 20 * scale, y + 13 * scale, 5 * scale, 4 * scale, CRAB);
    frame.rect(x + 7 * scale, y + 3 * scale, 3 * scale, 3 * scale, INK);
    frame.rect(x + 18 * scale, y + 3 * scale, 3 * scale, 3 * scale, INK);
}

pub(in crate::engine) fn draw_hero(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    scale: i32,
    state: &GameState,
) {
    if state.has_visual_template(VisualTemplate::Hero) {
        draw_oracle_hero(frame, x, y, scale, state);
        return;
    }
    let accent = HERO_STYLE_COLORS[state.hero_style];
    draw_crab(frame, x, y, scale);
    frame.rect(x + 8 * scale, y + 8 * scale, 12 * scale, 3 * scale, accent);
}

pub(in crate::engine) fn draw_oracle_hero(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    scale: i32,
    state: &GameState,
) {
    frame.blit_rgba(
        ORACLE_HEROES[state.hero_style],
        HERO_SPRITE_WIDTH,
        HERO_SPRITE_HEIGHT,
        x,
        y,
        scale,
    );
}

pub(in crate::engine) fn draw_defeated_hero(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    state: &GameState,
) {
    draw_hero(frame, x, y, 1, state);
    frame.rect(x + 5, y + 12, 14, 2, VOID);
    frame.line(x + 4, y + 35, x + 20, y + 35, ASH);
    frame.line(x - 7, y + 36, x + 31, y + 36, INDIGO);
}

pub(in crate::engine) fn draw_boss(frame: &mut Framebuffer, x: i32, y: i32, tick: u64, scale: i32) {
    let bob = ((tick / 15) % 2) as i32 * scale;
    frame.rect(x, y + bob, 30 * scale, 27 * scale, PLUM);
    frame.rect(
        x - 4 * scale,
        y + 7 * scale + bob,
        38 * scale,
        13 * scale,
        PLUM,
    );
    frame.rect(
        x + 5 * scale,
        y + 7 * scale + bob,
        5 * scale,
        5 * scale,
        GOLD,
    );
    frame.rect(
        x + 20 * scale,
        y + 7 * scale + bob,
        5 * scale,
        5 * scale,
        GOLD,
    );
    frame.rect(
        x + 8 * scale,
        y + 20 * scale + bob,
        14 * scale,
        3 * scale,
        RED,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_choices_preserve_authored_hero_art_while_aura_changes_colorway() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        finish_opening(&mut engine);
        for button in [Button::Start, Button::A] {
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: true,
                },
            );
            issue(
                &mut engine,
                EngineCommand::Input {
                    button,
                    pressed: false,
                },
            );
        }
        assert_eq!(engine.screen(), Screen::CharacterCreation);

        let default_name = hero_pixels(&engine);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        assert_eq!(hero_pixels(&engine), default_name);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: false,
            },
        );

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Down,
                pressed: true,
            },
        );
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Down,
                pressed: false,
            },
        );
        let default_path = hero_pixels(&engine);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        assert_eq!(hero_pixels(&engine), default_path);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: false,
            },
        );

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Down,
                pressed: true,
            },
        );
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Down,
                pressed: false,
            },
        );
        let default_style = hero_pixels(&engine);
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        assert_ne!(hero_pixels(&engine), default_style);
    }
}
