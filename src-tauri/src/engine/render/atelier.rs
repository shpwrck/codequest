use super::*;

pub(in crate::engine) const ATELIER_HEADER_BOX: UiBox = UiBox {
    x: 6,
    y: 4,
    width: 106,
    height: 16,
};
pub(in crate::engine) const ATELIER_ROW_BOXES: [UiBox; 3] = [
    UiBox {
        x: 116,
        y: 17,
        width: 98,
        height: 29,
    },
    UiBox {
        x: 116,
        y: 49,
        width: 98,
        height: 29,
    },
    UiBox {
        x: 116,
        y: 81,
        width: 98,
        height: 29,
    },
];

pub(in crate::engine) const fn atelier_label_box(row_box: UiBox) -> UiBox {
    UiBox {
        x: row_box.x + 4,
        y: row_box.y + 5,
        width: row_box.width - 8,
        height: 7,
    }
}

pub(in crate::engine) const fn atelier_value_box(row_box: UiBox) -> UiBox {
    UiBox {
        x: row_box.x + 4,
        y: row_box.y + 15,
        width: row_box.width - 8,
        height: 9,
    }
}

pub(in crate::engine) const ATELIER_BIND_BOX: UiBox = UiBox {
    x: 137,
    y: 112,
    width: 55,
    height: 21,
};
pub(in crate::engine) const ATELIER_BIND_TEXT_BOX: UiBox = UiBox {
    x: 137,
    y: 113,
    width: 55,
    height: 12,
};
pub(in crate::engine) const ATELIER_HERO_X: i32 = 32;
pub(in crate::engine) const ATELIER_HERO_Y: i32 = 38;
pub(in crate::engine) const ATELIER_HERO_SCALE: i32 = 2;

pub(in crate::engine) fn render_character_creation(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Atelier) {
        render_oracle_atelier(frame, state);
        return;
    }
    frame.clear(NAVY);
    frame.centered_text(9, "CREATE YOUR HERO", GOLD, 1);
    let bob = ((state.motion_ticks() / 20) % 2) as i32;
    draw_hero(frame, 26, 64 - bob, 1, state);

    let rows = [
        format!("NAME   < {} >", HERO_NAMES[state.hero_name]),
        format!("CLASS  < {} >", HERO_CLASSES[state.hero_class]),
        format!("STYLE  < {} >", HERO_STYLES[state.hero_style]),
        "BEGIN QUEST".to_string(),
    ];
    for (index, label) in rows.iter().enumerate() {
        let y = 30 + index as i32 * 21;
        if state.hero_row == index {
            frame.rect(65, y - 3, 169, 14, ROYAL);
            frame.text(69, y, ">", GOLD, 1);
        }
        frame.text(81, y, label, PARCH, 1);
    }
    frame.centered_text(120, creation_oracle_status(state, false), SKY, 1);
    frame.centered_text(138, "D-PAD:EDIT  A:CHOOSE", PARCH, 1);
    frame.centered_text(150, "START:BEGIN  B:BACK", MIST, 1);
}

/// Hero-creation status for the next run's first question, in the Oracle's
/// truthful states: ready, loading, a scheduled retry, or about to contact.
pub(in crate::engine) fn creation_oracle_status(state: &GameState, atelier: bool) -> &'static str {
    match (
        state.has_next_question(),
        state.questions_loading,
        state.question_retry_ticks > 0,
        atelier,
    ) {
        (true, ..) => "ORACLE READY",
        (_, true, _, true) => "ORACLE IS WRITING",
        (_, true, _, false) => "ORACLE IS WRITING...",
        (_, _, true, true) => "VISION CLOUDY - RETRYING",
        (_, _, true, false) => "ORACLE WILL RETRY...",
        (.., true) => "CONTACTING ORACLE",
        _ => "CONTACTING ORACLE...",
    }
}

pub(in crate::engine) fn render_oracle_atelier(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_ATELIER);
    frame.rect(
        ATELIER_HEADER_BOX.x,
        ATELIER_HEADER_BOX.y,
        ATELIER_HEADER_BOX.width,
        ATELIER_HEADER_BOX.height,
        VOID,
    );
    frame.outline(
        ATELIER_HEADER_BOX.x,
        ATELIER_HEADER_BOX.y,
        ATELIER_HEADER_BOX.width,
        ATELIER_HEADER_BOX.height,
        CYAN_DIM,
    );
    frame.centered_compact_text_box(ATELIER_HEADER_BOX, "BIND YOUR CODE-SEER", AMBER);
    let bob = ((state.motion_ticks() / 22) % 2) as i32;
    draw_hero(
        frame,
        ATELIER_HERO_X,
        ATELIER_HERO_Y - bob,
        ATELIER_HERO_SCALE,
        state,
    );

    let rows = [
        ("NAME", HERO_NAMES[state.hero_name]),
        ("PATH", HERO_CLASSES[state.hero_class]),
        ("AURA", HERO_STYLES[state.hero_style]),
    ];
    for (index, (label, value)) in rows.iter().enumerate() {
        let row_box = ATELIER_ROW_BOXES[index];
        let label_box = atelier_label_box(row_box);
        let value_box = atelier_value_box(row_box);
        let focused = state.hero_row == index;
        if focused {
            draw_asset_focus(frame, row_box.x, row_box.y, row_box.width, row_box.height);
        }
        frame.centered_text_box(label_box, label, if focused { AMBER } else { CYAN_DIM }, 1);
        frame.centered_compact_text_box(
            value_box,
            &format!("<{}>", truncate(value, 14)),
            if focused { PARCH } else { MIST },
        );
    }

    if state.hero_row == 3 {
        draw_asset_focus(
            frame,
            ATELIER_BIND_BOX.x,
            ATELIER_BIND_BOX.y,
            ATELIER_BIND_BOX.width,
            ATELIER_BIND_BOX.height,
        );
    }
    frame.centered_text_box(
        ATELIER_BIND_TEXT_BOX,
        "BIND",
        if state.hero_row == 3 { PARCH } else { MIST },
        1,
    );

    frame.rect(0, 143, WIDTH as i32, 17, VOID);
    frame.text(5, 148, creation_oracle_status(state, true), CYAN, 1);
    frame.text(174, 148, "START:BIND", MIST, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atelier_hero_visible_feet_rest_on_the_stage_support_line() {
        const STAGE_SUPPORT_Y: i32 = 106;

        for hero in ORACLE_HEROES {
            let visible = alpha_bounds(hero, HERO_SPRITE_WIDTH, HERO_SPRITE_HEIGHT);
            let visible_bottom =
                ATELIER_HERO_Y + (visible.y + visible.height) * ATELIER_HERO_SCALE - 1;
            assert_eq!(
                visible_bottom,
                STAGE_SUPPORT_Y - 1,
                "the hero's visible feet must meet the atelier platform instead of sinking into it"
            );
        }
    }

    #[test]
    fn hero_creation_status_names_the_oracles_real_state() {
        let mut cartridge = quiz_cartridge();
        cartridge.questions.clear();
        let mut state = GameState {
            cartridge: Some(cartridge),
            ..Default::default()
        };
        assert_eq!(creation_oracle_status(&state, true), "CONTACTING ORACLE");
        assert_eq!(
            creation_oracle_status(&state, false),
            "CONTACTING ORACLE..."
        );
        state.question_retry_ticks = 12;
        assert_eq!(
            creation_oracle_status(&state, true),
            "VISION CLOUDY - RETRYING"
        );
        assert_eq!(
            creation_oracle_status(&state, false),
            "ORACLE WILL RETRY..."
        );
        state.questions_loading = true;
        assert_eq!(creation_oracle_status(&state, true), "ORACLE IS WRITING");
        state.cartridge.as_mut().unwrap().questions = vec![concept_question(0)];
        assert_eq!(creation_oracle_status(&state, true), "ORACLE READY");
        // A question the session already answered is not ready for a new run.
        state.consumed_questions = 1;
        assert_eq!(creation_oracle_status(&state, true), "ORACLE IS WRITING");
    }
}
