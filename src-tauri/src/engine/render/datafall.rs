use super::*;

/// Header band heights of the Datafall scenes with one row, and with the
/// Oracle line beneath it. The tall bands end above the highest drop.
pub(in crate::engine) const SANCTUM_HEADER_HEIGHT: i32 = 15;
pub(in crate::engine) const SANCTUM_TALL_HEADER_HEIGHT: i32 = 22;
pub(in crate::engine) const LEGACY_HEADER_HEIGHT: i32 = 12;
pub(in crate::engine) const LEGACY_TALL_HEADER_HEIGHT: i32 = 21;
/// Where each Datafall scene writes its Oracle line.
pub(in crate::engine) const SANCTUM_ORACLE_LINE_BOX: UiBox = UiBox {
    x: 5,
    y: 13,
    width: 230,
    height: 7,
};
pub(in crate::engine) const LEGACY_ORACLE_LINE_BOX: UiBox = UiBox {
    x: 4,
    y: 12,
    width: 232,
    height: 7,
};

/// Rendered width of text segments drawn one space apart.
pub(in crate::engine) fn segments_width(segments: &[(String, Color)]) -> i32 {
    let characters = segments
        .iter()
        .map(|(text, _)| text.chars().count())
        .sum::<usize>()
        + segments.len().saturating_sub(1);
    text_width(&" ".repeat(characters), 1)
}

/// The Oracle line as colored segments that fit `width` pixels at full
/// letter spacing. A recalled lesson drops its lens label before any of its
/// answer would be cut.
pub(in crate::engine) fn oracle_line_segments(
    line: &OracleLine,
    width: i32,
) -> Vec<(String, Color)> {
    match line {
        OracleLine::Failure { reason, retry_in } => vec![
            (reason.clone(), AMBER),
            (
                match retry_in {
                    Some(seconds) => format!("- RETRY IN {seconds}S"),
                    None => "- RETRYING".into(),
                },
                MIST,
            ),
        ],
        OracleLine::Recall {
            outstanding,
            concept,
            answer,
        } => {
            let tag = if *outstanding {
                ("REVIEW", AMBER)
            } else {
                ("RECALL", CYAN)
            };
            let mut segments = vec![(tag.0.to_string(), tag.1)];
            if let Some(concept) = concept {
                segments.push((format!("{}:", concept.label()), MIST));
            }
            segments.push((truncate(answer.trim(), QUIZ_CHOICE_CHARS), PARCH));
            if segments.len() == 3 && segments_width(&segments) > width {
                segments.remove(1);
            }
            segments
        }
    }
}

/// Draws the Oracle line inside `bounds`, as many characters of it as
/// [`GameState::oracle_line_reveal`] allows this tick.
pub(in crate::engine) fn draw_oracle_line(
    frame: &mut Framebuffer,
    bounds: UiBox,
    state: &GameState,
    line: &OracleLine,
) {
    let segments = oracle_line_segments(line, bounds.width);
    debug_assert!(
        segments_width(&segments) <= bounds.width,
        "Oracle line {line:?} does not fit {bounds:?}"
    );
    let mut remaining = state.oracle_line_reveal(line);
    let mut x = bounds.x;
    for (text, color) in segments {
        if remaining == 0 {
            break;
        }
        let shown = truncate(&text, remaining);
        frame.text(x, bounds.y, &shown, color, 1);
        remaining = remaining.saturating_sub(text.chars().count() + 1);
        x += (text.chars().count() as i32 + 1) * GLYPH_ADVANCE;
    }
}

pub(in crate::engine) fn render_oracle(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Sanctum) {
        render_oracle_sanctum(frame, state);
        return;
    }
    frame.clear(INK);
    for index in 0..30 {
        let x = ((index * 71 + state.motion_ticks() as usize * 2) % WIDTH) as i32;
        frame.pixel(x, 14 + (index * 29 % 96) as i32, MIST);
    }
    let oracle_line = state.oracle_line();
    let header_height = if oracle_line.is_some() {
        LEGACY_TALL_HEADER_HEIGHT
    } else {
        LEGACY_HEADER_HEIGHT
    };
    frame.rect(0, 0, WIDTH as i32, header_height, NAVY);
    frame.rect(0, header_height - 1, WIDTH as i32, 1, PLUM);
    frame.text(4, 2, "ORACLE DATAFALL", GOLD, 1);
    if let Some(line) = &oracle_line {
        draw_oracle_line(frame, LEGACY_ORACLE_LINE_BOX, state, line);
    }
    let status = match state.question_status() {
        QuestionStatus::Ready => "QUESTION READY".to_string(),
        QuestionStatus::Writing => format!("{} THINKING", state.ai_provider_name()),
        QuestionStatus::Retrying => format!("{} RETRYING", state.ai_provider_name()),
        QuestionStatus::Contacting => format!("CONTACTING {}", state.ai_provider_name()),
    };
    let status_width = status.chars().count() as i32 * GLYPH_ADVANCE - 1;
    frame.text(211 - status_width, 2, &status, SKY, 1);
    let phase = if state.reduced_motion {
        2
    } else {
        (state.screen_ticks % 45) / 15
    };
    frame.text(216, 2, &".".repeat(phase as usize + 1), GOLD, 1);
    for drop in &state.oracle_drops {
        match drop.kind {
            OracleDropKind::Data => draw_oracle_data(frame, drop.x, drop.y),
            OracleDropKind::Bug => draw_oracle_bug(frame, drop.x, drop.y),
        }
    }
    frame.rect(0, 132, WIDTH as i32, 28, PLUM);
    frame.rect(0, 128, WIDTH as i32, 4, GREEN);
    draw_hero(frame, state.oracle_hero_x, 111, 1, state);
    frame.text(
        4,
        143,
        &format!("DATA {:02}", state.oracle_data.min(99)),
        GREEN,
        1,
    );
    let data_stage = threshold_stage(state.oracle_data, &DATA_CHARGE_THRESHOLDS);
    draw_oracle_rune_meter(
        frame,
        49,
        143,
        data_stage,
        if data_stage >= 2 { AMBER } else { CYAN },
    );
    frame.centered_text(143, "L/R MOVE  B:BACK", PARCH, 1);
    let breach_stage = threshold_stage(state.oracle_bug_hits, &BUG_BREACH_THRESHOLDS);
    let containment_color = match breach_stage {
        0 => CYAN,
        1 => AMBER,
        2 => MAGENTA,
        _ => RED,
    };
    draw_oracle_rune_meter(
        frame,
        173,
        143,
        3usize.saturating_sub(breach_stage),
        containment_color,
    );
    frame.text(
        201,
        143,
        &format!("BUG {:02}", state.oracle_bug_hits.min(99)),
        RED,
        1,
    );
}

pub(in crate::engine) fn render_oracle_sanctum(frame: &mut Framebuffer, state: &GameState) {
    let tier = state.visual_tier();
    match tier {
        PresentationTier::Initiate => frame.blit_rgb_graded(ORACLE_SANCTUM, 218, 238, 172),
        PresentationTier::Adept => frame.blit_rgb_graded(ORACLE_SANCTUM, 236, 250, 226),
        PresentationTier::OracleBound => frame.blit_rgb(ORACLE_SANCTUM),
    };
    let oracle_line = state.oracle_line();
    let (header_height, header_y) = if oracle_line.is_some() {
        (SANCTUM_TALL_HEADER_HEIGHT, 4)
    } else {
        (SANCTUM_HEADER_HEIGHT, 5)
    };
    frame.rect(0, 0, WIDTH as i32, header_height, VOID);
    if oracle_line.is_some() {
        // The tall band covers the plate's own header rule; restate it.
        frame.rect(0, header_height - 1, WIDTH as i32, 1, NAVY);
    }
    frame.rect(0, 143, WIDTH as i32, 17, VOID);
    frame.text(5, header_y, "DATAFALL", AMBER, 1);
    frame.text(
        60,
        header_y,
        tier.label(),
        match tier {
            PresentationTier::Initiate => CYAN_DIM,
            PresentationTier::Adept => AMBER,
            PresentationTier::OracleBound => MAGENTA,
        },
        1,
    );
    let status = state.ai_provider_status(match state.question_status() {
        QuestionStatus::Ready => "READY",
        QuestionStatus::Writing => "SCRYING",
        QuestionStatus::Retrying => "CLOUDY",
        QuestionStatus::Contacting => "CHANNEL",
    });
    let status_width = status.chars().count() as i32 * GLYPH_ADVANCE - 1;
    frame.text(235 - status_width, header_y, &status, CYAN, 1);
    if let Some(line) = &oracle_line {
        draw_oracle_line(frame, SANCTUM_ORACLE_LINE_BOX, state, line);
    }

    if tier == PresentationTier::OracleBound {
        for (x, y) in [
            (120, 28),
            (104, 38),
            (136, 38),
            (96, 52),
            (144, 52),
            (104, 66),
            (136, 66),
            (120, 76),
        ] {
            frame.rect(x - 2, y - 2, 5, 5, VOID);
            frame.outline(x - 2, y - 2, 5, 5, VIOLET);
            frame.pixel(x, y, MAGENTA);
        }
    }

    for drop in &state.oracle_drops {
        match drop.kind {
            OracleDropKind::Data => draw_oracle_data(frame, drop.x, drop.y),
            OracleDropKind::Bug => draw_oracle_bug(frame, drop.x, drop.y),
        }
    }

    draw_hero(frame, state.oracle_hero_x, 106, 1, state);
    frame.text(
        5,
        149,
        &format!("DATA {:02}", state.oracle_data.min(99)),
        GREEN,
        1,
    );
    let data_stage = threshold_stage(state.oracle_data, &DATA_CHARGE_THRESHOLDS);
    draw_oracle_rune_meter(
        frame,
        49,
        149,
        data_stage,
        if data_stage >= 2 { AMBER } else { CYAN },
    );
    frame.centered_text(149, "L/R:MOVE  B:LEAVE", PARCH, 1);
    let breach_stage = threshold_stage(state.oracle_bug_hits, &BUG_BREACH_THRESHOLDS);
    let containment_color = match breach_stage {
        0 => CYAN,
        1 => AMBER,
        2 => MAGENTA,
        _ => RED,
    };
    draw_oracle_rune_meter(
        frame,
        173,
        149,
        3usize.saturating_sub(breach_stage),
        containment_color,
    );
    frame.text(
        198,
        149,
        &format!("BUG {:02}", state.oracle_bug_hits.min(99)),
        RED,
        1,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_crossings_change_datafall_runes_at_the_declared_breakpoints() {
        let mut state = GameState {
            cartridge: Some(oracle_template_cartridge()),
            questions_loading: true,
            oracle_data: 2,
            oracle_bug_hits: 0,
            ..Default::default()
        };
        let mut before = Framebuffer::default();
        render_oracle_sanctum(&mut before, &state);

        state.oracle_data = 3;
        state.oracle_bug_hits = 1;
        let mut after = Framebuffer::default();
        render_oracle_sanctum(&mut after, &state);

        assert_ne!(
            frame_region(&before.pixels, 49..68, 149..156),
            frame_region(&after.pixels, 49..68, 149..156),
            "the first data charge threshold must light a rune"
        );
        assert_ne!(
            frame_region(&before.pixels, 173..192, 149..156),
            frame_region(&after.pixels, 173..192, 149..156),
            "the first corruption threshold must break a containment rune"
        );
    }

    #[test]
    fn oracle_hud_separates_oracle_info_at_top_from_game_info_at_bottom() {
        let engine = waiting_oracle_engine();
        let frame = engine.frame();
        for (label, color) in [("Oracle title/progress", GOLD), ("AI provider status", SKY)] {
            assert!(
                color_pixels_in_region(frame, color, 0..WIDTH, 0..12) > 0,
                "{label} is missing from the top quiz HUD"
            );
        }
        for (label, color) in [
            ("data counter", GREEN),
            ("controls", PARCH),
            ("bug counter", RED),
        ] {
            assert!(
                color_pixels_in_region(frame, color, 0..WIDTH, 132..HEIGHT) > 0,
                "{label} is missing from the bottom game HUD"
            );
        }
        assert_eq!(color_pixels_in_region(frame, GREEN, 0..WIDTH, 0..12), 0);
        assert_eq!(color_pixels_in_region(frame, RED, 0..WIDTH, 0..12), 0);
    }

    #[test]
    fn question_failures_reach_the_oracle_line_and_clear_on_success() {
        let mut engine = waiting_oracle_engine();
        let current = engine_state(&engine).question_request_seq;
        assert!(engine_state(&engine).questions_loading);

        // A superseded request's failure, or another cartridge's, says
        // nothing about the request still in flight.
        issue(
            &mut engine,
            EngineCommand::Questions {
                cartridge_id: "/tmp/engine-test".into(),
                result: Err("TIMED OUT".into()),
                seq: current.wrapping_sub(1),
            },
        );
        fail(&mut engine, "/tmp/other-cartridge", "TIMED OUT");
        let state = engine_state(&engine);
        assert!(state.questions_loading, "the current request is in flight");
        assert_eq!(state.question_failure, None);
        assert_eq!(state.question_retry_ticks, 0);
        assert_eq!(state.oracle_line(), None);

        fail(&mut engine, "/tmp/engine-test", "timed out");
        let state = engine_state(&engine);
        assert!(!state.questions_loading);
        assert_eq!(state.question_failure.as_deref(), Some("TIMED OUT"));
        assert_eq!(
            state.oracle_line(),
            Some(OracleLine::Failure {
                reason: "TIMED OUT".into(),
                retry_in: Some(5),
            })
        );
        assert_eq!(oracle_line_texts(state), ["TIMED OUT", "- RETRY IN 5S"]);
        let frame = engine.frame();
        let line = LEGACY_ORACLE_LINE_BOX;
        let line_x = line.x as usize..(line.x + line.width) as usize;
        let line_y = line.y as usize..(line.y + line.height) as usize;
        assert!(
            color_pixels_in_region(frame, AMBER, line_x.clone(), line_y.clone()) > 0,
            "the Datafall header names the failure"
        );

        let mut requests = Vec::new();
        for _ in 0..300 {
            engine.update();
            requests.extend(request_seqs(&engine.take_effects()));
        }
        assert_eq!(requests.len(), 1, "the existing retry delay still holds");
        let state = engine_state(&engine);
        assert!(state.questions_loading);
        assert_eq!(
            oracle_line_texts(state),
            ["TIMED OUT", "- RETRYING"],
            "without a journal the line keeps the last reason while retrying"
        );

        deliver(&mut engine, "/tmp/engine-test", vec![concept_question(0)]);
        let state = engine_state(&engine);
        assert_eq!(state.question_failure, None, "success clears the reason");
        assert_eq!(state.oracle_line(), None);
        let mut cleared = Framebuffer::default();
        render_oracle(&mut cleared, state);
        assert_eq!(
            color_pixels_in_region(&cleared.pixels, AMBER, line_x, line_y),
            0,
            "the header returns to one row"
        );
    }

    #[test]
    fn the_oracle_recalls_missed_lessons_first_and_cycles_the_journal() {
        let mut state = waiting_state_with(vec![
            recall_lesson("OLDEST CLEARED", Some(Concept::Purpose), false),
            recall_lesson("OLDER MISS", Some(Concept::Interaction), true),
            recall_lesson("NEWER CLEARED", None, false),
            recall_lesson("NEWEST MISS", Some(Concept::Invariant), true),
        ]);
        let answer_at = |state: &mut GameState, ticks: u64| {
            state.screen_ticks = ticks;
            state.recalled_lesson().map(|lesson| lesson.answer.clone())
        };
        let expected = [
            "NEWEST MISS",
            "OLDER MISS",
            "NEWER CLEARED",
            "OLDEST CLEARED",
        ];
        for (slot, answer) in expected.iter().enumerate() {
            let start = slot as u64 * ORACLE_RECALL_TICKS;
            assert_eq!(answer_at(&mut state, start).as_deref(), Some(*answer));
            assert_eq!(
                answer_at(&mut state, start + ORACLE_RECALL_TICKS - 1).as_deref(),
                Some(*answer),
                "each lesson holds for its whole slot"
            );
        }
        assert_eq!(
            answer_at(&mut state, 4 * ORACLE_RECALL_TICKS).as_deref(),
            Some("NEWEST MISS"),
            "the cycle wraps back to the top"
        );

        state.screen_ticks = 0;
        assert_eq!(
            oracle_line_texts(&state),
            ["REVIEW", "INVARIANTS:", "NEWEST MISS"]
        );
        state.screen_ticks = 2 * ORACLE_RECALL_TICKS;
        assert_eq!(oracle_line_texts(&state), ["RECALL", "NEWER CLEARED"]);

        // A failure waiting out its retry delay takes the line and keeps it
        // for the retry's first second; a retry still in flight after that
        // gives the line back to the journal.
        state.question_failure = Some("RATE LIMITED".into());
        state.questions_loading = false;
        state.question_retry_ticks = 61;
        assert_eq!(oracle_line_texts(&state), ["RATE LIMITED", "- RETRY IN 2S"]);
        state.question_retry_ticks = 0;
        state.questions_loading = true;
        state.question_request_ticks = ORACLE_RETRY_HOLD_TICKS - 1;
        assert_eq!(oracle_line_texts(&state), ["RATE LIMITED", "- RETRYING"]);
        state.question_request_ticks = ORACLE_RETRY_HOLD_TICKS;
        assert_eq!(oracle_line_texts(&state), ["RECALL", "NEWER CLEARED"]);

        // A ready question has no failure to explain, but recall continues.
        state.question_retry_ticks = 120;
        state.cartridge.as_mut().unwrap().questions = vec![concept_question(0)];
        assert_eq!(oracle_line_texts(&state), ["RECALL", "NEWER CLEARED"]);

        assert_eq!(waiting_state_with(Vec::new()).oracle_line(), None);
    }

    #[test]
    fn recalled_lessons_are_written_in_but_stay_static_under_reduced_motion() {
        let header = |frame: &Framebuffer| {
            frame.pixels[..WIDTH * SANCTUM_TALL_HEADER_HEIGHT as usize * 4].to_vec()
        };
        let mut state = waiting_state_with(journal_lessons());
        let slot_start = ORACLE_RECALL_TICKS;
        state.screen_ticks = slot_start;
        let line = state.oracle_line().unwrap();
        assert_eq!(
            state.oracle_line_reveal(&line),
            ORACLE_RECALL_REVEAL_PER_TICK
        );
        let mut arriving = Framebuffer::default();
        render_oracle_sanctum(&mut arriving, &state);
        maybe_write_preview("06f-sanctum-recall-arriving", &arriving.pixels);
        state.screen_ticks = slot_start + 60;
        let mut settled = Framebuffer::default();
        render_oracle_sanctum(&mut settled, &state);
        assert_ne!(
            header(&arriving),
            header(&settled),
            "a new lesson is written in with motion"
        );

        state.reduced_motion = true;
        assert_eq!(state.oracle_line_reveal(&line), usize::MAX);
        for ticks in [
            slot_start,
            slot_start + 1,
            slot_start + 30,
            slot_start + 239,
        ] {
            state.screen_ticks = ticks;
            let mut still = Framebuffer::default();
            render_oracle_sanctum(&mut still, &state);
            assert_eq!(
                header(&still),
                header(&settled),
                "reduced motion shows the whole line at once and holds it still"
            );
        }
    }

    #[test]
    fn the_oracle_line_stays_contained_disjoint_and_readable() {
        let screen = LayoutBounds {
            x: 0,
            y: 0,
            width: WIDTH as i32,
            height: HEIGHT as i32,
        };
        let longest_answer = "W".repeat(QUIZ_CHOICE_CHARS);
        let worst_lines = [
            OracleLine::Failure {
                reason: "W".repeat(ORACLE_FAILURE_CHARS),
                retry_in: Some(5),
            },
            OracleLine::Failure {
                reason: "W".repeat(ORACLE_FAILURE_CHARS),
                retry_in: None,
            },
            OracleLine::Recall {
                outstanding: true,
                concept: Some(Concept::Invariant),
                answer: longest_answer.clone(),
            },
            OracleLine::Recall {
                outstanding: false,
                concept: Some(Concept::Invariant),
                answer: "W".repeat(18),
            },
        ];
        // The lens is kept whenever it fits beside the answer.
        assert_eq!(
            oracle_line_segments(&worst_lines[3], SANCTUM_ORACLE_LINE_BOX.width).len(),
            3
        );
        assert_eq!(
            oracle_line_segments(&worst_lines[2], SANCTUM_ORACLE_LINE_BOX.width)
                .last()
                .map(|(text, _)| text.clone()),
            Some(longest_answer.clone()),
            "the answer is never cut"
        );

        let drop_offset = DROP_SPRITE_SIZE as i32 / 2;
        for (scene, band_height, band_color, line_box, row_one, hero) in [
            (
                "sanctum",
                SANCTUM_TALL_HEADER_HEIGHT,
                VOID,
                SANCTUM_ORACLE_LINE_BOX,
                vec![
                    text_bounds(5, 4, "DATAFALL", 1),
                    text_bounds(60, 4, "ORACLE-BOUND", 1),
                    text_bounds(
                        235 - text_width("CLAUDE:CHANNEL", 1),
                        4,
                        "CLAUDE:CHANNEL",
                        1,
                    ),
                ],
                LayoutBounds {
                    x: ORACLE_HERO_MIN_X,
                    y: 106,
                    width: ORACLE_HERO_MAX_X + HERO_SPRITE_WIDTH as i32 - ORACLE_HERO_MIN_X,
                    height: HERO_SPRITE_HEIGHT as i32,
                },
            ),
            (
                "legacy",
                LEGACY_TALL_HEADER_HEIGHT,
                NAVY,
                LEGACY_ORACLE_LINE_BOX,
                vec![
                    text_bounds(4, 2, "ORACLE DATAFALL", 1),
                    text_bounds(
                        211 - text_width("CONTACTING CLAUDE", 1),
                        2,
                        "CONTACTING CLAUDE",
                        1,
                    ),
                    text_bounds(216, 2, "...", 1),
                ],
                LayoutBounds {
                    x: ORACLE_HERO_MIN_X,
                    y: 111,
                    width: ORACLE_HERO_MAX_X + 28 - ORACLE_HERO_MIN_X,
                    height: 17,
                },
            ),
        ] {
            let band = LayoutBounds {
                x: 0,
                y: 0,
                width: WIDTH as i32,
                height: band_height,
            };
            let line = ui_box_bounds(line_box);
            assert!(bounds_contains(screen, band), "{scene} band");
            assert!(bounds_contains(band, line), "{scene} line inside its band");
            for (index, element) in row_one.iter().enumerate() {
                assert!(bounds_contains(band, *element), "{scene} row one {index}");
                assert!(
                    bounds_are_disjoint(*element, line),
                    "{scene} row one {index} overlaps the line"
                );
            }
            assert!(
                bounds_are_disjoint(band, hero),
                "{scene} band meets the hero"
            );
            for lane in [24, 54, 82, 112, 142, 210] {
                let highest_drop = LayoutBounds {
                    x: lane - drop_offset,
                    y: 30 - drop_offset,
                    width: DROP_SPRITE_SIZE as i32,
                    height: DROP_SPRITE_SIZE as i32,
                };
                assert!(
                    bounds_are_disjoint(band, highest_drop),
                    "{scene} band would hide a falling drop in lane {lane}"
                );
            }
            for worst in &worst_lines {
                let segments = oracle_line_segments(worst, line_box.width);
                let drawn = LayoutBounds {
                    x: line_box.x,
                    y: line_box.y,
                    width: segments_width(&segments),
                    height: 7,
                };
                assert!(
                    bounds_contains(line, drawn),
                    "{scene} {worst:?} overflows its line"
                );
                for (text, color) in &segments {
                    assert!(
                        contrast_ratio(*color, band_color) >= 4.5,
                        "{scene} `{text}` is not readable on its band"
                    );
                }

                // Every pixel the line draws stays inside its box.
                let mut isolated = Framebuffer::default();
                let state = GameState {
                    reduced_motion: true,
                    ..Default::default()
                };
                draw_oracle_line(&mut isolated, line_box, &state, worst);
                for (index, pixel) in isolated.pixels.as_chunks::<4>().0.iter().enumerate() {
                    if pixel[3] == 0 {
                        continue;
                    }
                    let (x, y) = ((index % WIDTH) as i32, (index / WIDTH) as i32);
                    assert!(
                        bounds_contains(
                            line,
                            LayoutBounds {
                                x,
                                y,
                                width: 1,
                                height: 1,
                            }
                        ),
                        "{scene} drew outside its line at ({x}, {y})"
                    );
                }
            }
        }

        // With a line, nothing below the tall band changes: drops, hero,
        // plate, and footer are exactly what the one-row scene shows.
        let mut quiet = waiting_state_with(Vec::new());
        quiet.oracle_drops = vec![OracleDrop {
            x: 112,
            y: 30,
            kind: OracleDropKind::Data,
        }];
        let mut recalling = waiting_state_with(vec![recall_lesson(
            &longest_answer,
            Some(Concept::Invariant),
            true,
        )]);
        recalling.oracle_drops = quiet.oracle_drops.clone();
        recalling.screen_ticks = 60;
        let mut cloudy = waiting_state_with(journal_lessons());
        cloudy.oracle_drops = quiet.oracle_drops.clone();
        cloudy.questions_loading = false;
        cloudy.question_retry_ticks = 240;
        cloudy.question_failure = Some("TIMED OUT".into());
        let mut quiet_frame = Framebuffer::default();
        render_oracle_sanctum(&mut quiet_frame, &quiet);
        let below = SANCTUM_TALL_HEADER_HEIGHT as usize * WIDTH * 4;
        for (name, state) in [
            ("06c-sanctum-recall", &recalling),
            ("06d-sanctum-cloudy", &cloudy),
        ] {
            let mut frame = Framebuffer::default();
            render_oracle_sanctum(&mut frame, state);
            maybe_write_preview(name, &frame.pixels);
            assert_eq!(
                frame.pixels[below..],
                quiet_frame.pixels[below..],
                "{name} changed the playfield"
            );
        }

        let legacy_state = |lessons: Vec<Lesson>| {
            let mut cartridge = quiz_cartridge();
            cartridge.questions.clear();
            cartridge.lessons = lessons;
            GameState {
                cartridge: Some(cartridge),
                oracle_drops: recalling.oracle_drops.clone(),
                screen_ticks: 60,
                ..waiting_state_with(Vec::new())
            }
        };
        let legacy_quiet = legacy_state(Vec::new());
        let legacy_recalling = legacy_state(journal_lessons());
        let mut legacy_quiet_frame = Framebuffer::default();
        render_oracle(&mut legacy_quiet_frame, &legacy_quiet);
        let mut legacy_frame = Framebuffer::default();
        render_oracle(&mut legacy_frame, &legacy_recalling);
        maybe_write_preview("06e-datafall-recall", &legacy_frame.pixels);
        let legacy_below = LEGACY_TALL_HEADER_HEIGHT as usize * WIDTH * 4;
        assert_eq!(
            legacy_frame.pixels[legacy_below..],
            legacy_quiet_frame.pixels[legacy_below..],
            "the legacy line changed the playfield"
        );
    }

    #[test]
    fn oracle_progression_changes_the_sanctum_without_relying_on_level_text() {
        let mut state = GameState {
            cartridge: Some(oracle_template_cartridge()),
            quiz: Some(QuizRun::new()),
            questions_loading: true,
            screen_ticks: 90,
            ..Default::default()
        };

        let mut initiate = Framebuffer::default();
        render_oracle_sanctum(&mut initiate, &state);
        state.quiz.as_mut().unwrap().level = 2;
        let mut adept = Framebuffer::default();
        render_oracle_sanctum(&mut adept, &state);
        state.quiz.as_mut().unwrap().level = 4;
        let mut oracle_bound = Framebuffer::default();
        render_oracle_sanctum(&mut oracle_bound, &state);

        assert_ne!(initiate.pixels, adept.pixels);
        assert_ne!(adept.pixels, oracle_bound.pixels);
        assert_eq!(
            color_pixels_in_region(&adept.pixels, MAGENTA, 0..WIDTH, 14..100),
            0,
            "Adept should not borrow the final tier's magenta crest"
        );
        assert!(
            color_pixels_in_region(&oracle_bound.pixels, MAGENTA, 0..WIDTH, 14..100) > 0,
            "Oracle-bound should add a final non-numeric visual channel"
        );
    }
}
