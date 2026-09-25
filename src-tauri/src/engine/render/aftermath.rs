use super::*;

pub(in crate::engine) const AFTERMATH_CONTENT_BOX: UiBox = UiBox {
    x: 141,
    y: 18,
    width: 85,
    height: 116,
};

pub(in crate::engine) fn render_game_over(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Aftermath) {
        render_oracle_aftermath(frame, state);
        return;
    }
    frame.clear(INK);
    frame.outline(8, 8, 224, 144, PLUM);
    frame.centered_text(24, "GAME OVER", RED, 2);
    if let Some(run) = state.quiz.as_ref() {
        let insight = InsightStage::from_score(run.score);
        let mut rows = vec![
            (format!("SCORE {:04}", run.score.min(9999)), GOLD),
            (format!("INSIGHT {}", insight.label()), insight.color()),
            (format!("LEVEL {} REACHED", run.level.min(99)), SKY),
        ];
        rows.extend(ledger_rows(state, run));
        for ((text, color), y) in rows.iter().zip(GAME_OVER_ROW_YS) {
            frame.centered_text(y, text, *color, 1);
        }
        draw_oracle_sigil(frame, GAME_OVER_SIGIL_X, 110, 0);
        draw_hero(frame, GAME_OVER_SIGIL_X - 12, 99, 1, state);
    } else {
        frame.centered_text(70, "NO QUESTIONS FOUND", GOLD, 1);
    }
    frame.centered_text(143, RESULT_PROMPT, PARCH, 1);
}

/// Every input the result screen accepts; drawn steadily, never blinking.
pub(in crate::engine) const RESULT_PROMPT: &str = "A/B/START:MENU";
/// Legacy result rows: score, insight, level, then the learning ledger.
pub(in crate::engine) const GAME_OVER_ROW_YS: [i32; 7] = [46, 56, 66, 80, 90, 100, 110];
/// The legacy result hero stands left of the centered ledger.
pub(in crate::engine) const GAME_OVER_SIGIL_X: i32 = 42;
pub(in crate::engine) const AFTERMATH_TITLE_Y: i32 = 24;
/// Aftermath rows: score and insight, tier and level, then the ledger.
pub(in crate::engine) const AFTERMATH_ROW_YS: [i32; 8] = [36, 46, 58, 68, 80, 90, 100, 110];
pub(in crate::engine) const AFTERMATH_PROMPT_Y: i32 = 124;

/// Lessons in the journal whose latest attempt was a miss.
pub(in crate::engine) fn open_reviews(state: &GameState) -> usize {
    state
        .lessons()
        .iter()
        .filter(|lesson| lesson.outstanding)
        .count()
}

/// The lens whose mastery rose the most since the run began, with its stage
/// now; ties go to the earliest lens in `Concept::ALL`.
pub(in crate::engine) fn woken_lens(state: &GameState) -> Option<(Concept, usize)> {
    let run = state.quiz.as_ref()?;
    let mut woken: Option<(Concept, usize, usize)> = None;
    for (concept, start) in Concept::ALL.into_iter().zip(run.ledger.stages_at_start) {
        let stage = state.mastery_stage(concept);
        let rise = stage.saturating_sub(start);
        if rise > 0 && woken.is_none_or(|(_, _, best)| rise > best) {
            woken = Some((concept, stage, rise));
        }
    }
    woken.map(|(concept, stage, _)| (concept, stage))
}

/// The run's learning ledger, shared by both result screens: first-try
/// successes, redemptions, open reviews, and where to go next (the lens that
/// woke this run, else the Codex while reviews are open).
pub(in crate::engine) fn ledger_rows(state: &GameState, run: &QuizRun) -> Vec<(String, Color)> {
    let ledger = &run.ledger;
    let open = open_reviews(state);
    let mut rows = vec![
        (
            first_try_label((ledger.first_try_right, ledger.first_try)),
            PARCH,
        ),
        (format!("REDEEMED {:02}", ledger.redeemed.min(99)), CYAN),
        if open == 0 {
            ("ALL CLEAR".into(), MIST)
        } else {
            (format!("REVIEW {:02}", open.min(99)), AMBER)
        },
    ];
    if let Some((concept, stage)) = woken_lens(state) {
        rows.push((
            format!("{} {}", concept.label(), rune_numeral(stage)),
            mastery_rune_color(stage),
        ));
    } else if open > 0 {
        rows.push(("SEE CODEX".into(), AMBER));
    }
    rows
}

/// The Aftermath panel's rows below its title, each with its baseline.
pub(in crate::engine) fn aftermath_rows(
    state: &GameState,
    run: &QuizRun,
) -> Vec<(i32, String, Color)> {
    let tier = state.visual_tier();
    let insight = InsightStage::from_score(run.score);
    let mut rows = vec![
        (format!("SCORE {:04}", run.score.min(9999)), AMBER),
        (format!("INSIGHT {}", insight.label()), insight.color()),
        (
            tier.label().to_string(),
            if tier == PresentationTier::Initiate {
                CYAN
            } else {
                AMBER
            },
        ),
        (format!("LEVEL {}", run.level.min(99)), PARCH),
    ];
    rows.extend(ledger_rows(state, run));
    rows.into_iter()
        .zip(AFTERMATH_ROW_YS)
        .map(|((text, color), y)| (y, text, color))
        .collect()
}

pub(in crate::engine) fn render_oracle_aftermath(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_AFTERMATH);
    frame.centered_text_in(
        AFTERMATH_CONTENT_BOX.x,
        AFTERMATH_TITLE_Y,
        AFTERMATH_CONTENT_BOX.width,
        "VISION CLOSED",
        RED,
        1,
    );
    if let Some(run) = state.quiz.as_ref() {
        for (y, text, color) in aftermath_rows(state, run) {
            frame.centered_text_in(
                AFTERMATH_CONTENT_BOX.x,
                y,
                AFTERMATH_CONTENT_BOX.width,
                &text,
                color,
                1,
            );
        }
        draw_defeated_hero(frame, 52, 106, state);
    } else {
        frame.centered_text_in(
            AFTERMATH_CONTENT_BOX.x,
            79,
            AFTERMATH_CONTENT_BOX.width,
            "NO QUESTIONS",
            AMBER,
            1,
        );
    }
    frame.centered_text_in(
        AFTERMATH_CONTENT_BOX.x,
        AFTERMATH_PROMPT_Y,
        AFTERMATH_CONTENT_BOX.width,
        RESULT_PROMPT,
        PARCH,
        1,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debrief_rows_stay_contained_disjoint_and_readable_on_their_panels() {
        let state = worst_case_debrief_state();
        let run = state.quiz.as_ref().unwrap();
        let panel = ui_box_bounds(AFTERMATH_CONTENT_BOX);
        let rows = aftermath_rows(&state, run);
        assert_eq!(
            rows.iter()
                .map(|(_, text, _)| text.as_str())
                .collect::<Vec<_>>(),
            [
                "SCORE 0000",
                "INSIGHT UNLIT",
                "ORACLE-BOUND",
                "LEVEL 99",
                "1ST TRY 99/99",
                "REDEEMED 99",
                "REVIEW 99",
                "INVARIANTS III",
            ],
            "the worst case fills every ledger row"
        );
        let mut children = vec![(
            "title".to_string(),
            centered_text_in_bounds(panel, AFTERMATH_TITLE_Y, "VISION CLOSED", 1),
            vec![RED],
        )];
        for (y, text, color) in &rows {
            // Every color the row can take: the insight, tier, and lens rows
            // change color with their stage.
            let colors = vec![*color, CYAN, AMBER, MAGENTA, MIST, PARCH];
            children.push((
                text.clone(),
                centered_text_in_bounds(panel, *y, text, 1),
                colors,
            ));
        }
        // Alternative copy for a row, at that row's baseline.
        for (row, alternative) in [
            (0, "SCORE 9999"),
            (1, "INSIGHT III"),
            (2, "INITIATE"),
            (6, "ALL CLEAR"),
            (7, "SEE CODEX"),
        ] {
            let bounds = centered_text_in_bounds(panel, rows[row].0, alternative, 1);
            assert!(
                bounds_contains(panel, bounds),
                "{alternative} exceeds the aftermath panel"
            );
        }
        children.push((
            "prompt".to_string(),
            centered_text_in_bounds(panel, AFTERMATH_PROMPT_Y, RESULT_PROMPT, 1),
            vec![PARCH],
        ));
        for (name, bounds, colors) in &children {
            assert!(
                bounds_contains(panel, *bounds),
                "aftermath {name} {bounds:?} exceeds the panel {panel:?}"
            );
            let fill = brightest_plate_color(ORACLE_AFTERMATH, *bounds);
            for color in colors {
                let ratio = contrast_ratio(*color, fill);
                assert!(
                    ratio >= 4.5,
                    "aftermath {name} contrast {ratio:.2}:1 against plate fill {:?} is below 4.5:1",
                    (fill.0, fill.1, fill.2)
                );
            }
        }
        for (index, (left_name, left, _)) in children.iter().enumerate() {
            for (right_name, right, _) in &children[index + 1..] {
                assert!(
                    bounds_are_disjoint(*left, *right),
                    "aftermath {left_name} {left:?} overlaps {right_name} {right:?}"
                );
            }
        }

        // The legacy result: centered rows inside the frame, clear of the
        // hero and sigil standing to their left, readable on ink.
        let frame_interior = LayoutBounds {
            x: 9,
            y: 9,
            width: 222,
            height: 142,
        };
        let hero = LayoutBounds {
            x: GAME_OVER_SIGIL_X - 12,
            y: 99,
            width: HERO_SPRITE_WIDTH as i32,
            height: HERO_SPRITE_HEIGHT as i32,
        };
        let sigil = LayoutBounds {
            x: GAME_OVER_SIGIL_X - 24,
            y: 110 - 13,
            width: 49,
            height: 27,
        };
        let mut legacy = vec![
            (
                "heading".to_string(),
                centered_text_bounds(24, "GAME OVER", 2),
            ),
            (
                "prompt".to_string(),
                centered_text_bounds(143, RESULT_PROMPT, 1),
            ),
        ];
        let legacy_copy = [
            "SCORE 9999",
            "INSIGHT UNLIT",
            "LEVEL 99 REACHED",
            "1ST TRY 99/99",
            "REDEEMED 99",
            "REVIEW 99",
            "INVARIANTS III",
        ];
        for (text, y) in legacy_copy.into_iter().zip(GAME_OVER_ROW_YS) {
            legacy.push((text.to_string(), centered_text_bounds(y, text, 1)));
        }
        for (name, bounds) in &legacy {
            assert!(
                bounds_contains(frame_interior, *bounds),
                "legacy {name} {bounds:?} exceeds the frame"
            );
            assert!(
                bounds_are_disjoint(*bounds, hero),
                "legacy {name} overlaps the hero"
            );
            assert!(
                bounds_are_disjoint(*bounds, sigil),
                "legacy {name} overlaps the sigil"
            );
        }
        for (index, (left_name, left)) in legacy.iter().enumerate() {
            for (right_name, right) in &legacy[index + 1..] {
                assert!(
                    bounds_are_disjoint(*left, *right),
                    "legacy {left_name} overlaps {right_name}"
                );
            }
        }
        for color in [GOLD, SKY, PARCH, CYAN, MIST, AMBER, MAGENTA] {
            let ratio = contrast_ratio(color, INK);
            assert!(
                ratio >= 4.5,
                "legacy {:?} on ink is {ratio:.2}:1",
                (color.0, color.1, color.2)
            );
        }

        // The Ascension recap boxes and the legacy level-up lines.
        let batch = inset(ASCENSION_BATCH_BOX);
        let footer = inset(MENU_FOOTER_BOX);
        let recap = centered_text_box_bounds(ASCENSION_BATCH_BOX, "1ST TRY 99/99", 1);
        assert!(bounds_contains(batch, recap), "{recap:?} exceeds {batch:?}");
        let hero_column = LayoutBounds {
            x: 108,
            y: 0,
            width: HERO_SPRITE_WIDTH as i32,
            height: HEIGHT as i32,
        };
        assert!(bounds_are_disjoint(
            ui_box_bounds(ASCENSION_BATCH_BOX),
            hero_column
        ));
        assert!(bounds_are_disjoint(
            ui_box_bounds(ASCENSION_BATCH_BOX),
            ui_box_bounds(ASCENSION_LEVEL_BOX)
        ));
        for level in 2..=5 {
            let next = next_focus_label(level);
            let bounds = centered_text_box_bounds(MENU_FOOTER_BOX, &next, 1);
            assert!(bounds_contains(footer, bounds), "{next} exceeds the footer");
            let legacy_next = centered_text_bounds(LEVEL_UP_FOOTER_Y, &next, 1);
            assert!(
                bounds_contains(frame_interior, legacy_next),
                "{next} exceeds the frame"
            );
            assert!(bounds_are_disjoint(
                legacy_next,
                centered_text_bounds(130, "1ST TRY 99/99", 1)
            ));
        }
        assert!(bounds_are_disjoint(
            centered_text_bounds(22, "LEVEL UP!", 2),
            centered_text_bounds(42, "ORACLE BOND DEEPENS", 1)
        ));
        assert!(
            centered_text_bounds(42, "ORACLE BOND DEEPENS", 1).y + 7 < 72 - 13,
            "the legacy bond title clears the sigil"
        );
        for (name, foreground, background) in [
            ("recap", PARCH, VOID),
            ("next lenses", CYAN, VOID),
            ("legacy bond title", CYAN, NAVY),
            ("legacy recap", CYAN, NAVY),
            ("legacy next lenses", MIST, NAVY),
        ] {
            let ratio = contrast_ratio(foreground, background);
            assert!(ratio >= 4.5, "{name} contrast {ratio:.2}:1 is below 4.5:1");
        }
    }

    #[test]
    fn the_result_prompt_stays_lit_on_every_frame() {
        for template in [true, false] {
            let mut state = worst_case_debrief_state();
            if !template {
                state.cartridge = Some(quiz_cartridge());
            }
            let (x_range, prompt_y) = if template {
                let panel = AFTERMATH_CONTENT_BOX;
                (
                    panel.x as usize..(panel.x + panel.width) as usize,
                    AFTERMATH_PROMPT_Y as usize,
                )
            } else {
                (0..WIDTH, 143)
            };
            let y_range = prompt_y..prompt_y + 7;
            for ticks in [0, 29, 30, 45, 60, 95] {
                state.screen_ticks = ticks;
                let mut frame = Framebuffer::default();
                if template {
                    render_oracle_aftermath(&mut frame, &state);
                } else {
                    render_game_over(&mut frame, &state);
                }
                assert!(
                    color_pixels_in_region(&frame.pixels, PARCH, x_range.clone(), y_range.clone())
                        > 20,
                    "the result prompt is drawn at tick {ticks} (template={template})"
                );
                if ticks == 30 {
                    let name = if template {
                        "debrief-aftermath-worst"
                    } else {
                        "debrief-game-over-legacy"
                    };
                    maybe_write_preview(name, &frame.pixels);
                }
            }
            state.quiz.as_mut().unwrap().level = 3;
            let mut level_up = Framebuffer::default();
            let name = if template {
                render_oracle_ascension(&mut level_up, &state);
                "debrief-ascension-deepens"
            } else {
                render_level_up(&mut level_up, &state);
                "debrief-level-up-legacy"
            };
            maybe_write_preview(name, &level_up.pixels);
        }
    }

    #[test]
    fn debrief_screens_draw_the_rows_they_report() {
        let plain = |color| {
            let mut frame = Framebuffer::default();
            frame.clear(color);
            frame
        };
        let mut state = worst_case_debrief_state();
        // A batch recap distinct from the run totals, so the screens must read
        // the completed batch's snapshot.
        state.quiz.as_mut().unwrap().ledger.last_batch = (4, 6);

        // The Aftermath draws every row it computes, lens row included.
        let mut plate = Framebuffer::default();
        plate.blit_rgb(ORACLE_AFTERMATH);
        let mut frame = Framebuffer::default();
        render_oracle_aftermath(&mut frame, &state);
        let panel = ui_box_bounds(AFTERMATH_CONTENT_BOX);
        let rows = aftermath_rows(&state, state.quiz.as_ref().unwrap());
        assert_eq!(rows.len(), AFTERMATH_ROW_YS.len());
        for (y, text, color) in &rows {
            assert_text_drawn(
                &frame,
                &plate,
                centered_text_in_bounds(panel, *y, text, 1),
                text,
                *color,
            );
        }

        // The Ascension heading follows the tier crossing, and the recap and
        // hold footer name the batch just survived and the lenses ahead.
        let void = plain(VOID);
        for (level, title) in [(3, "ORACLE BOND DEEPENS"), (4, "ORACLE BOND ASCENDS")] {
            state.quiz.as_mut().unwrap().level = level;
            let mut frame = Framebuffer::default();
            render_oracle_ascension(&mut frame, &state);
            assert_text_drawn(
                &frame,
                &void,
                centered_text_in_bounds(ui_box_bounds(ASCENSION_TITLE_BOX), 73, title, 1),
                title,
                AMBER,
            );
            assert_text_drawn(
                &frame,
                &void,
                centered_text_box_bounds(ASCENSION_BATCH_BOX, "1ST TRY 4/6", 1),
                "1ST TRY 4/6",
                PARCH,
            );
            let next = next_focus_label(level);
            assert_text_drawn(
                &frame,
                &void,
                centered_text_box_bounds(MENU_FOOTER_BOX, &next, 1),
                &next,
                CYAN,
            );
        }

        // The legacy screens carry the same debrief.
        let template = state.cartridge.take().unwrap();
        let mut legacy = quiz_cartridge();
        legacy.lessons = template.lessons;
        legacy.mastery = template.mastery;
        state.cartridge = Some(legacy);
        state.quiz.as_mut().unwrap().level = 3;
        let mut frame = Framebuffer::default();
        render_level_up(&mut frame, &state);
        let navy = plain(NAVY);
        let next = next_focus_label(3);
        for (y, text, color) in [
            (42, "ORACLE BOND DEEPENS", CYAN),
            (130, "1ST TRY 4/6", CYAN),
            (LEVEL_UP_FOOTER_Y, next.as_str(), MIST),
        ] {
            assert_text_drawn(&frame, &navy, centered_text_bounds(y, text, 1), text, color);
        }

        let mut frame = Framebuffer::default();
        render_game_over(&mut frame, &state);
        let run = state.quiz.as_ref().unwrap();
        let insight = InsightStage::from_score(run.score);
        let mut rows = vec![
            ("SCORE 0000".to_string(), GOLD),
            ("INSIGHT UNLIT".to_string(), insight.color()),
            ("LEVEL 3 REACHED".to_string(), SKY),
        ];
        rows.extend(ledger_rows(&state, run));
        assert_eq!(rows.len(), GAME_OVER_ROW_YS.len(), "the lens row is shown");
        let ink = plain(INK);
        for ((text, color), y) in rows.iter().zip(GAME_OVER_ROW_YS) {
            assert_text_drawn(&frame, &ink, centered_text_bounds(y, text, 1), text, *color);
        }
    }

    #[test]
    fn woken_lens_names_the_largest_rise_since_the_run_began() {
        let mut state = GameState {
            cartridge: Some(quiz_cartridge()),
            ..Default::default()
        };
        start_quiz_run(&mut state);
        assert_eq!(woken_lens(&state), None, "no evidence, no rise");

        let cartridge = state.cartridge.as_mut().unwrap();
        cartridge.mastery = Mastery::from([
            (
                Concept::Purpose,
                LensRecord {
                    first_try: 1,
                    ..LensRecord::default()
                },
            ),
            (
                Concept::Interaction,
                LensRecord {
                    first_try: 3,
                    ..LensRecord::default()
                },
            ),
            (
                Concept::Tradeoff,
                LensRecord {
                    redeemed: 3,
                    ..LensRecord::default()
                },
            ),
        ]);
        assert_eq!(
            woken_lens(&state),
            Some((Concept::Interaction, 2)),
            "the largest rise wins and ties go to the earlier lens"
        );

        // A new run starts from the stages already lit: nothing has woken yet.
        start_quiz_run(&mut state);
        assert_eq!(
            state.quiz.as_ref().unwrap().ledger.stages_at_start,
            [1, 0, 2, 0, 2]
        );
        assert_eq!(woken_lens(&state), None);
    }
}
