use super::*;

pub(in crate::engine) const CODEX_HEADING_BOX: UiBox = UiBox {
    x: 62,
    y: 40,
    width: 116,
    height: 7,
};
pub(in crate::engine) const CODEX_LENS_ROW_Y: i32 = 54;
pub(in crate::engine) const CODEX_LENS_ROW_PITCH: i32 = 11;
pub(in crate::engine) const CODEX_LENS_LABEL_X: i32 = 69;
pub(in crate::engine) const CODEX_LENS_RUNES_X: i32 = 133;
pub(in crate::engine) const CODEX_LENS_PENDING_X: i32 = 159;
pub(in crate::engine) const CODEX_TOTALS_BOX: UiBox = UiBox {
    x: 42,
    y: 122,
    width: 156,
    height: 17,
};
pub(in crate::engine) const CODEX_PROMPT_BOX: UiBox = UiBox {
    x: 42,
    y: 141,
    width: 156,
    height: 16,
};
pub(in crate::engine) const CODEX_LESSON_HEADER_BOX: UiBox = UiBox {
    x: 37,
    y: 2,
    width: 203,
    height: 28,
};
pub(in crate::engine) const CODEX_QUESTION_BOX: UiBox = UiBox {
    x: 17,
    y: 31,
    width: 197,
    height: 38,
};
/// Lesson panels start clear of the trial plate's portrait frame and end
/// before its brazier (question) or pillar (answer and rationale).
pub(in crate::engine) const CODEX_ANSWER_BOX: UiBox = UiBox {
    x: 17,
    y: 71,
    width: 212,
    height: 15,
};
pub(in crate::engine) const CODEX_RATIONALE_BOX: UiBox = UiBox {
    x: 17,
    y: 88,
    width: 212,
    height: 66,
};
pub(in crate::engine) const CODEX_TEXT_X: i32 = 23;
pub(in crate::engine) const CODEX_ANSWER_TEXT_X: i32 = 31;
pub(in crate::engine) const CODEX_QUESTION_Y: i32 = 35;
pub(in crate::engine) const CODEX_ANSWER_Y: i32 = 75;
pub(in crate::engine) const CODEX_WHY_Y: i32 = 103;
pub(in crate::engine) const CODEX_RATIONALE_Y: i32 = 116;
/// A sealed lesson's self-test: the `YOU CHOSE` heading, then the pick and
/// the first misconception row, each `CODEX_CHOSE_GAP` pixels apart.
pub(in crate::engine) const CODEX_CHOSE_Y: i32 = 99;
pub(in crate::engine) const CODEX_CHOSE_GAP: i32 = 11;

pub(in crate::engine) fn mastery_rune_color(stage: usize) -> Color {
    match stage {
        0 | 1 => CYAN,
        2 => AMBER,
        _ => MAGENTA,
    }
}

pub(in crate::engine) fn pending_reviews(lessons: &[Lesson], concept: Concept) -> usize {
    lessons
        .iter()
        .filter(|lesson| lesson.outstanding && lesson.concept == Some(concept))
        .count()
}

pub(in crate::engine) fn codex_lesson_counter(index: usize, total: usize) -> String {
    format!("LESSON {:02}/{:02}", (index + 1).min(999), total.min(999))
}

pub(in crate::engine) fn codex_lesson_status(lesson: &Lesson) -> (&'static str, Color) {
    match (lesson.outstanding, lesson.spaced_check, lesson.peeked) {
        (true, _, true) => ("PENDING PEEKED", AMBER),
        (true, _, false) => ("REVIEW PENDING", AMBER),
        (false, true, true) => ("CHECK PEEKED", AMBER),
        (false, true, false) => ("CHECK PENDING", AMBER),
        (false, false, _) => ("LEARNED", CYAN),
    }
}

/// The answer panel's prompt while a pending answer is sealed.
pub(in crate::engine) const CODEX_REVEAL_PROMPT: &str = "A:REVEAL ANSWER";
/// The self-test prompt for a sealed lesson that has no recorded pick.
pub(in crate::engine) const CODEX_THINK_PROMPT: &str = "THINK, THEN A:REVEAL";
pub(in crate::engine) const CODEX_CHOSE_HEADING: &str = "YOU CHOSE";
pub(in crate::engine) const CODEX_ONCE_CHOSE: &str = "ONCE CHOSE: ";

/// The player's recorded wrong pick, marked `-` so the misconception never
/// depends on color alone.
pub(in crate::engine) fn codex_pick_line(pick: &str) -> String {
    format!("- {}", truncate(pick, QUIZ_CHOICE_CHARS))
}

/// A learned lesson's one-line reminder of the misconception it replaced. A
/// pick too long for one rationale row is cut at a word boundary and marked
/// `...`, so it never reads as a different, shorter choice.
pub(in crate::engine) fn codex_once_chose_line(pick: &str) -> String {
    let line = format!("{CODEX_ONCE_CHOSE}{pick}");
    if line.chars().count() <= RATIONALE_COLUMNS {
        return line;
    }
    let cut = wrap_text(&line, RATIONALE_COLUMNS - 3)
        .into_iter()
        .next()
        .unwrap_or_default();
    format!("{cut}...")
}

/// Draws a sealed lesson's self-test at `y`: the player's pick and the
/// misconception it reveals, or a prompt to recall the answer first when the
/// save recorded no pick. The heading, the pick, and the misconception's first
/// row start `gap` pixels apart.
pub(in crate::engine) fn draw_codex_self_test(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    gap: i32,
    lesson: &Lesson,
) {
    let Some((pick, why)) = lesson.misconception.as_ref() else {
        frame.text(x, y, CODEX_THINK_PROMPT, MIST, 1);
        return;
    };
    frame.text(x, y, CODEX_CHOSE_HEADING, CYAN_DIM, 1);
    frame.text(x, y + gap, &codex_pick_line(pick), RED, 1);
    let why_y = y + 2 * gap;
    if why.trim().is_empty() {
        frame.text(x, why_y, CODEX_THINK_PROMPT, MIST, 1);
    } else {
        frame.wrapped_text(x, why_y, why, MIST, RATIONALE_COLUMNS, RATIONALE_ROWS);
    }
}

/// Draws the `ONCE CHOSE` reminder on a learned lesson's row below its
/// rationale, when the lesson remembers a misconception.
pub(in crate::engine) fn draw_codex_once_chose(
    frame: &mut Framebuffer,
    x: i32,
    rationale_y: i32,
    lesson: &Lesson,
) {
    if lesson.outstanding {
        return;
    }
    if let Some((pick, _)) = lesson.misconception.as_ref() {
        let y = rationale_y + RATIONALE_ROWS as i32 * LINE_HEIGHT;
        frame.text(x, y, &codex_once_chose_line(pick), MIST, 1);
    }
}

/// Learned and pending-review halves of the Codex totals line.
pub(in crate::engine) fn codex_totals(lessons: &[Lesson]) -> (String, String, Color) {
    let pending = lessons.iter().filter(|lesson| lesson.outstanding).count();
    let learned = format!("LEARNED {:02}", (lessons.len() - pending).min(99));
    if pending == 0 {
        (learned, "ALL CLEAR".into(), MIST)
    } else {
        (learned, format!("REVIEW {:02}", pending.min(99)), AMBER)
    }
}

/// The legend the Codex totals row shows while a lens with a pending review
/// has a cracked rune.
pub(in crate::engine) const CODEX_CRACKED_LEGEND: &str = "CRACKED = REVIEW DUE";
/// The legend while every cracked rune is held back by recent accuracy alone:
/// there is no review to take, only fresh answers to get right.
pub(in crate::engine) const CODEX_SLIPPED_LEGEND: &str = "CRACKED = LOW ACCURACY";

/// The cracked-rune legend the totals row in `bounds` shows instead of the
/// learned and pending counts: only while a rune is cracked, and only when
/// the legend fits inside the row's panel border.
pub(in crate::engine) fn codex_totals_show_legend(
    bounds: UiBox,
    legend: Option<&str>,
) -> Option<&str> {
    legend.filter(|legend| text_width(legend, 1) <= bounds.width - 4)
}

pub(in crate::engine) fn draw_codex_totals(
    frame: &mut Framebuffer,
    bounds: UiBox,
    lessons: &[Lesson],
    legend: Option<&str>,
) {
    if let Some(legend) = codex_totals_show_legend(bounds, legend) {
        frame.centered_text_box(bounds, legend, AMBER, 1);
        return;
    }
    let (learned, review, review_color) = codex_totals(lessons);
    let width = text_width(&format!("{learned}  {review}"), 1);
    let x = bounds.x + (bounds.width - width) / 2;
    let y = bounds.y + (bounds.height - 7) / 2;
    frame.text(x, y, &learned, CYAN, 1);
    let review_x = x + (learned.chars().count() as i32 + 2) * GLYPH_ADVANCE;
    frame.text(review_x, y, &review, review_color, 1);
}

pub(in crate::engine) fn draw_codex_panel(frame: &mut Framebuffer, bounds: UiBox) {
    frame.rect(bounds.x, bounds.y, bounds.width, bounds.height, VOID);
    frame.outline(bounds.x, bounds.y, bounds.width, bounds.height, CYAN_DIM);
}

/// Draws a lesson's lens label ending just left of its three mastery runes at
/// `runes_x`; a lesson without a lens shows `GENERAL` and no runes.
pub(in crate::engine) fn draw_codex_lens(
    frame: &mut Framebuffer,
    runes_x: i32,
    y: i32,
    concept: Option<Concept>,
    state: &GameState,
) {
    let Some(concept) = concept else {
        let label_x = runes_x + 19 - text_width("GENERAL", 1);
        frame.text(label_x, y, "GENERAL", MIST, 1);
        return;
    };
    let stage = state.mastery_stage(concept);
    let label_x = runes_x - 6 - text_width(concept.label(), 1);
    frame.text(label_x, y, concept.label(), PARCH, 1);
    draw_mastery_meter(
        frame,
        runes_x,
        y,
        stage,
        state.mastery_cracks(concept),
        mastery_rune_color(stage),
    );
}

pub(in crate::engine) fn draw_codex_rationale(
    frame: &mut Framebuffer,
    x: i32,
    y: i32,
    lesson: &Lesson,
) {
    if lesson.rationale.trim().is_empty() {
        frame.text(x, y, "NO RATIONALE WAS RECORDED", MIST, 1);
    } else {
        frame.wrapped_text(
            x,
            y,
            &lesson.rationale,
            PARCH,
            RATIONALE_COLUMNS,
            RATIONALE_ROWS,
        );
    }
}

pub(in crate::engine) fn render_codex(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Codex) {
        render_oracle_codex(frame, state);
        return;
    }
    let Some((index, lesson)) = state.codex_lesson() else {
        render_codex_mastery(frame, state);
        return;
    };
    frame.clear(NAVY);
    frame.rect(0, 0, WIDTH as i32, 16, INK);
    frame.text(
        5,
        4,
        &codex_lesson_counter(index, state.lessons().len()),
        SKY,
        1,
    );
    draw_codex_lens(frame, 214, 4, lesson.concept, state);
    let (status, status_color) = codex_lesson_status(lesson);
    frame.text(5, 21, status, status_color, 1);
    frame.rect(5, 31, 230, 38, INK);
    frame.outline(5, 31, 230, 38, SKY);
    frame.wrapped_text(
        11,
        35,
        &lesson.question,
        PARCH,
        QUIZ_QUESTION_COLUMNS,
        QUIZ_QUESTION_ROWS,
    );
    frame.rect(5, 72, 230, 15, INK);
    if state.codex_answer_sealed(lesson) {
        // The sealed self-test sits on the lesson card's void panel, where
        // the misconception's red stays readable.
        frame.text(9, 76, CODEX_REVEAL_PROMPT, MIST, 1);
        frame.rect(5, 90, 230, 52, VOID);
        frame.outline(5, 90, 230, 52, MIST);
        draw_codex_self_test(frame, 11, 95, 10, lesson);
    } else {
        frame.text(9, 76, ">", GOLD, 1);
        frame.text(
            21,
            76,
            &truncate(&lesson.answer, QUIZ_CHOICE_CHARS),
            GREEN,
            1,
        );
        frame.rect(5, 90, 230, 52, INK);
        frame.outline(5, 90, 230, 52, MIST);
        frame.text(11, 95, "WHY IT HOLDS", SKY, 1);
        draw_codex_rationale(frame, 11, 107, lesson);
        draw_codex_once_chose(frame, 11, 107, lesson);
    }
    frame.text(5, 151, "L/R:PAGE", MIST, 1);
    frame.text(199, 151, "B:BACK", MIST, 1);
}

pub(in crate::engine) fn render_codex_mastery(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(NAVY);
    frame.rect(0, 0, WIDTH as i32, 16, INK);
    frame.text(5, 4, "ORACLE CODEX", GOLD, 1);
    frame.text(195, 4, "MASTERY", SKY, 1);
    frame.outline(30, 24, 180, 82, SKY);
    let lessons = state.lessons();
    for (row, concept) in Concept::ALL.into_iter().enumerate() {
        let y = 32 + row as i32 * 14;
        let stage = state.mastery_stage(concept);
        frame.text(
            42,
            y,
            concept.label(),
            if stage > 0 { PARCH } else { MIST },
            1,
        );
        draw_mastery_meter(
            frame,
            128,
            y,
            stage,
            state.mastery_cracks(concept),
            mastery_rune_color(stage),
        );
        let pending = pending_reviews(lessons, concept);
        if pending > 0 {
            frame.text(156, y, &format!("!{}", pending.min(99)), AMBER, 1);
        }
    }
    if lessons.is_empty() {
        frame.centered_text(116, "NO LESSONS YET", GOLD, 1);
        frame.centered_text(130, "ANSWER TRIALS TO WRITE LESSONS", MIST, 1);
    } else {
        draw_codex_totals(
            frame,
            UiBox {
                x: 0,
                y: 116,
                width: WIDTH as i32,
                height: 7,
            },
            lessons,
            state.mastery_crack_legend(),
        );
        frame.text(5, 151, "L/R:PAGE", MIST, 1);
    }
    frame.text(199, 151, "B:BACK", MIST, 1);
}

pub(in crate::engine) fn render_oracle_codex(frame: &mut Framebuffer, state: &GameState) {
    let Some((index, lesson)) = state.codex_lesson() else {
        render_oracle_codex_mastery(frame, state);
        return;
    };
    frame.blit_rgb(ORACLE_TRIAL);
    frame.blit_rgba(
        ORACLE_PORTRAITS[state.hero_style],
        HERO_PORTRAIT_SIZE,
        HERO_PORTRAIT_SIZE,
        8,
        5,
        1,
    );
    draw_codex_panel(frame, CODEX_LESSON_HEADER_BOX);
    frame.text(
        44,
        7,
        &codex_lesson_counter(index, state.lessons().len()),
        CYAN,
        1,
    );
    draw_codex_lens(frame, 214, 7, lesson.concept, state);
    let (status, status_color) = codex_lesson_status(lesson);
    frame.text(44, 20, status, status_color, 1);
    frame.text(147, 20, "L/R:PAGE B:BACK", MIST, 1);

    draw_codex_panel(frame, CODEX_QUESTION_BOX);
    frame.wrapped_text(
        CODEX_TEXT_X,
        CODEX_QUESTION_Y,
        &lesson.question,
        PARCH,
        QUIZ_QUESTION_COLUMNS,
        QUIZ_QUESTION_ROWS,
    );

    draw_codex_panel(frame, CODEX_ANSWER_BOX);
    draw_codex_panel(frame, CODEX_RATIONALE_BOX);
    if state.codex_answer_sealed(lesson) {
        frame.text(CODEX_TEXT_X, CODEX_ANSWER_Y, CODEX_REVEAL_PROMPT, MIST, 1);
        draw_codex_self_test(frame, CODEX_TEXT_X, CODEX_CHOSE_Y, CODEX_CHOSE_GAP, lesson);
        return;
    }
    draw_oracle_rune(frame, CODEX_TEXT_X, CODEX_ANSWER_Y, true, GREEN);
    frame.text(
        CODEX_ANSWER_TEXT_X,
        CODEX_ANSWER_Y,
        &truncate(&lesson.answer, QUIZ_CHOICE_CHARS),
        GREEN,
        1,
    );
    frame.text(CODEX_TEXT_X, CODEX_WHY_Y, "WHY IT HOLDS", CYAN_DIM, 1);
    draw_codex_rationale(frame, CODEX_TEXT_X, CODEX_RATIONALE_Y, lesson);
    draw_codex_once_chose(frame, CODEX_TEXT_X, CODEX_RATIONALE_Y, lesson);
}

pub(in crate::engine) fn render_oracle_codex_mastery(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_CHRONICLE);
    frame.centered_text_box(CODEX_HEADING_BOX, "ORACLE CODEX", AMBER, 1);
    let lessons = state.lessons();
    for (row, concept) in Concept::ALL.into_iter().enumerate() {
        let y = CODEX_LENS_ROW_Y + row as i32 * CODEX_LENS_ROW_PITCH;
        let stage = state.mastery_stage(concept);
        frame.text(
            CODEX_LENS_LABEL_X,
            y,
            concept.label(),
            if stage > 0 { PARCH } else { MIST },
            1,
        );
        draw_mastery_meter(
            frame,
            CODEX_LENS_RUNES_X,
            y,
            stage,
            state.mastery_cracks(concept),
            mastery_rune_color(stage),
        );
        let pending = pending_reviews(lessons, concept);
        if pending > 0 {
            frame.text(
                CODEX_LENS_PENDING_X,
                y,
                &format!("!{}", pending.min(99)),
                AMBER,
                1,
            );
        }
    }
    draw_codex_panel(frame, CODEX_TOTALS_BOX);
    draw_codex_panel(frame, CODEX_PROMPT_BOX);
    if lessons.is_empty() {
        frame.centered_text_box(CODEX_TOTALS_BOX, "NO LESSONS YET", AMBER, 1);
        frame.centered_text_box(CODEX_PROMPT_BOX, "ANSWER A TRIAL  B:BACK", MIST, 1);
    } else {
        draw_codex_totals(
            frame,
            CODEX_TOTALS_BOX,
            lessons,
            state.mastery_crack_legend(),
        );
        frame.centered_text_box(CODEX_PROMPT_BOX, "L/R:READ LESSONS  B:BACK", MIST, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dogfood_manifest_routes_its_menu_into_the_templated_codex() {
        let mut cartridge = oracle_template_cartridge();
        cartridge.lessons = journal_lessons();
        cartridge.mastery = journal_mastery();
        let mut engine = quiz_menu_engine(cartridge);
        maybe_write_preview("oracle-menu-journal", engine.frame());
        press(&mut engine, Button::Down);
        maybe_write_preview("oracle-menu-codex-focus", engine.frame());

        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        let state = game_state(&engine);
        assert!(state.uses_visual_template(VisualTemplate::Codex));
        let mut expected = Framebuffer::default();
        render_oracle_codex(&mut expected, state);
        assert_eq!(engine.frame(), expected.pixels.as_slice());

        press(&mut engine, Button::Right);
        press(&mut engine, Button::Right);
        let state = game_state(&engine);
        assert_eq!(state.codex_lesson().map(|(index, _)| index), Some(1));
        let mut expected = Framebuffer::default();
        render_oracle_codex(&mut expected, state);
        assert_eq!(engine.frame(), expected.pixels.as_slice());
    }

    #[test]
    fn a_relearned_lesson_awaiting_its_spaced_check_is_sealed_and_its_reveal_is_recorded() {
        let mut cartridge = journal_cartridge();
        cartridge.lessons[0].spaced_check = true;
        let question = cartridge.lessons[0].question.clone();
        let mut engine = quiz_menu_engine(cartridge);
        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        let _ = engine.take_effects();
        let answer_green = |engine: &GameEngine| {
            color_pixels_in_region(
                engine.frame(),
                GREEN,
                CODEX_ANSWER_BOX.x as usize..(CODEX_ANSWER_BOX.x + CODEX_ANSWER_BOX.width) as usize,
                CODEX_ANSWER_BOX.y as usize
                    ..(CODEX_ANSWER_BOX.y + CODEX_ANSWER_BOX.height) as usize,
            )
        };
        press(&mut engine, Button::Right);
        let lesson = game_state(&engine).codex_lesson().unwrap().1.clone();
        assert!(!lesson.outstanding);
        assert_eq!(codex_lesson_status(&lesson).0, "CHECK PENDING");
        assert_eq!(
            answer_green(&engine),
            0,
            "a due spaced check is never open-book"
        );

        press(&mut engine, Button::A);
        assert!(answer_green(&engine) > 0);
        let effects = engine.take_effects();
        assert!(
            matches!(
                effects.as_slice(),
                [EngineEffect::MarkPeeked { question: peeked, .. }] if *peeked == question
            ),
            "{effects:?}"
        );
        let lesson = &game_state(&engine).lessons()[0];
        assert!(lesson.peeked);
        assert_eq!(codex_lesson_status(lesson).0, "CHECK PEEKED");
    }

    #[test]
    fn a_reveals_a_pending_answer_once_and_every_page_turn_seals_it_again() {
        let mut engine = quiz_menu_engine(journal_cartridge());
        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        let deck = game_state(&engine)
            .cartridge
            .as_ref()
            .unwrap()
            .questions
            .iter()
            .map(|question| (question.question.clone(), question.review))
            .collect::<Vec<_>>();
        let _ = engine.take_effects();
        let answer_green = |engine: &GameEngine| {
            color_pixels_in_region(
                engine.frame(),
                GREEN,
                CODEX_ANSWER_BOX.x as usize..(CODEX_ANSWER_BOX.x + CODEX_ANSWER_BOX.width) as usize,
                CODEX_ANSWER_BOX.y as usize
                    ..(CODEX_ANSWER_BOX.y + CODEX_ANSWER_BOX.height) as usize,
            )
        };

        press(&mut engine, Button::Right);
        press(&mut engine, Button::Right);
        assert_eq!(game_state(&engine).codex_lesson().unwrap().0, 1);
        assert_eq!(answer_green(&engine), 0, "a pending page opens sealed");

        press(&mut engine, Button::A);
        assert!(game_state(&engine).codex_revealed);
        assert!(answer_green(&engine) > 0, "A reveals the answer");
        let effects = engine.take_effects();
        assert_eq!(effects.len(), 1, "{effects:?}");
        assert!(matches!(
            &effects[0],
            EngineEffect::MarkPeeked { question, .. } if question == WORST_LESSON_QUESTION
        ));
        assert!(game_state(&engine).lessons()[1].peeked);

        press(&mut engine, Button::Start);
        assert!(
            engine.take_effects().is_empty(),
            "a second reveal records nothing"
        );

        press(&mut engine, Button::Right);
        press(&mut engine, Button::Left);
        assert_eq!(game_state(&engine).codex_lesson().unwrap().0, 1);
        assert_eq!(
            answer_green(&engine),
            0,
            "turning back seals the answer again"
        );
        press(&mut engine, Button::A);
        assert!(answer_green(&engine) > 0);
        assert!(
            engine.take_effects().is_empty(),
            "a lesson is marked peeked once"
        );

        press(&mut engine, Button::B);
        press(&mut engine, Button::Start);
        assert_eq!(engine.screen(), Screen::Codex);
        assert!(
            !game_state(&engine).codex_revealed,
            "every visit seals again"
        );

        let cartridge = game_state(&engine).cartridge.as_ref().unwrap();
        let after = cartridge
            .questions
            .iter()
            .map(|question| (question.question.clone(), question.review))
            .collect::<Vec<_>>();
        assert_eq!(after, deck, "the Codex never changes the question deck");
        assert_eq!(cartridge.mastery, journal_mastery());
    }

    #[test]
    fn empty_codex_says_no_lessons_yet_and_cannot_page() {
        let mut engine = quiz_menu_engine(quiz_cartridge());
        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        for button in [Button::Right, Button::Left, Button::R, Button::Down] {
            press(&mut engine, button);
            assert_eq!(game_state(&engine).codex_page, 0);
        }
        assert!(
            color_pixels_in_region(engine.frame(), GOLD, 0..WIDTH, 116..123) > 0,
            "the legacy Codex names its empty journal"
        );
        maybe_write_preview("codex-legacy-empty", engine.frame());

        let state = GameState {
            cartridge: Some(oracle_template_cartridge()),
            ..Default::default()
        };
        let mut empty = Framebuffer::default();
        render_oracle_codex(&mut empty, &state);
        maybe_write_preview("oracle-codex-empty", &empty.pixels);
        let mut expected = Framebuffer::default();
        expected.blit_rgb(ORACLE_CHRONICLE);
        draw_codex_panel(&mut expected, CODEX_TOTALS_BOX);
        expected.centered_text_box(CODEX_TOTALS_BOX, "NO LESSONS YET", AMBER, 1);
        draw_codex_panel(&mut expected, CODEX_PROMPT_BOX);
        expected.centered_text_box(CODEX_PROMPT_BOX, "ANSWER A TRIAL  B:BACK", MIST, 1);
        for bounds in [CODEX_TOTALS_BOX, CODEX_PROMPT_BOX] {
            let x = bounds.x as usize..(bounds.x + bounds.width) as usize;
            let y = bounds.y as usize..(bounds.y + bounds.height) as usize;
            assert_eq!(
                frame_region(&empty.pixels, x.clone(), y.clone()),
                frame_region(&expected.pixels, x, y),
                "the empty journal shows an honest state and how to fill it"
            );
        }
    }

    #[test]
    fn codex_mastery_runes_wake_at_the_exact_evidence_thresholds_on_the_rendered_frame() {
        let invariant_row = Concept::ALL
            .iter()
            .position(|concept| *concept == Concept::Invariant)
            .unwrap() as i32;
        for (name, renderer, runes_x, row_y) in [
            (
                "oracle",
                render_oracle_codex_mastery as fn(&mut Framebuffer, &GameState),
                CODEX_LENS_RUNES_X,
                CODEX_LENS_ROW_Y + invariant_row * CODEX_LENS_ROW_PITCH,
            ),
            ("legacy", render_codex_mastery, 128, 32 + invariant_row * 14),
        ] {
            let mut meters = Vec::new();
            for evidence in 0..=6u32 {
                let mut cartridge = quiz_cartridge();
                // Evidence mixes first-try successes and redemptions; misses
                // never light a rune.
                cartridge.mastery.insert(
                    Concept::Invariant,
                    LensRecord {
                        first_try: evidence - evidence / 2,
                        redeemed: evidence / 2,
                        missed: 9,
                        ..LensRecord::default()
                    },
                );
                let state = GameState {
                    cartridge: Some(cartridge),
                    ..Default::default()
                };
                let mut frame = Framebuffer::default();
                renderer(&mut frame, &state);
                let x = runes_x as usize..(runes_x + 19) as usize;
                let y = row_y as usize..(row_y + 7) as usize;
                let lit = color_pixels_in_region(&frame.pixels, PARCH, x.clone(), y.clone()) / 5;
                assert_eq!(
                    lit,
                    [0, 1, 1, 2, 2, 3, 3][evidence as usize],
                    "{name}: {evidence} evidence must light the declared rune count"
                );
                let stage_color = [None, Some(CYAN), Some(AMBER), Some(MAGENTA)][lit];
                if let Some(color) = stage_color {
                    assert!(
                        color_pixels_in_region(&frame.pixels, color, x.clone(), y.clone()) > 0,
                        "{name}: stage {lit} uses its own rune color"
                    );
                }
                meters.push(frame_region(&frame.pixels, x, y));
            }
            for (below, at) in [(0, 1), (2, 3), (4, 5)] {
                assert_ne!(
                    meters[below], meters[at],
                    "{name}: crossing {MASTERY_THRESHOLDS:?} changes the meter"
                );
            }
            for (at, above) in [(1, 2), (3, 4), (5, 6)] {
                assert_eq!(
                    meters[at], meters[above],
                    "{name}: the meter holds between thresholds"
                );
            }
        }
    }

    #[test]
    fn codex_mastery_runes_crack_at_the_exact_gate_breakpoints_on_the_rendered_frame() {
        use RuneStyle::{Cracked, Lit, Unlit};
        let invariant_row = Concept::ALL
            .iter()
            .position(|concept| *concept == Concept::Invariant)
            .unwrap() as i32;
        let lesson = |outstanding| Lesson {
            question: "WHAT MUST STAY TRUE WHEN A SCENE CHANGES?".into(),
            answer: "ONLY THE ENGINE CHANGES SCENES".into(),
            rationale: String::new(),
            concept: Some(Concept::Invariant),
            outstanding,
            misconception: None,
            peeked: false,
            spaced_check: false,
        };
        for (name, renderer, runes_x, row_y, totals) in [
            (
                "oracle",
                render_oracle_codex_mastery as fn(&mut Framebuffer, &GameState),
                CODEX_LENS_RUNES_X,
                CODEX_LENS_ROW_Y + invariant_row * CODEX_LENS_ROW_PITCH,
                CODEX_TOTALS_BOX,
            ),
            (
                "legacy",
                render_codex_mastery,
                128,
                32 + invariant_row * 14,
                UiBox {
                    x: 0,
                    y: 116,
                    width: WIDTH as i32,
                    height: 7,
                },
            ),
        ] {
            let render = |record: LensRecord, outstanding: bool| {
                let mut cartridge = oracle_template_cartridge();
                if name == "legacy" {
                    cartridge.codequest = None;
                }
                cartridge.mastery.insert(Concept::Invariant, record);
                cartridge.lessons = vec![lesson(outstanding)];
                let state = GameState {
                    cartridge: Some(cartridge),
                    ..Default::default()
                };
                let mut frame = Framebuffer::default();
                renderer(&mut frame, &state);
                (frame.pixels, state.mastery_crack_legend())
            };
            for (record, outstanding, expected) in [
                (gated_record(5), false, [Lit, Lit, Lit]),
                (gated_record(4), false, [Lit, Lit, Lit]),
                (gated_record(3), false, [Lit, Lit, Cracked]),
                (gated_record(2), false, [Lit, Cracked, Cracked]),
                (gated_record(0), false, [Lit, Cracked, Cracked]),
                (gated_record(5), true, [Lit, Lit, Cracked]),
                (gated_record(3), true, [Lit, Lit, Cracked]),
                (
                    LensRecord {
                        first_try: 3,
                        ..gated_record(0)
                    },
                    false,
                    [Lit, Cracked, Unlit],
                ),
            ] {
                let (frame, crack_legend) = render(record, outstanding);
                let cracked = crack_legend.is_some();
                assert_eq!(
                    meter_styles(&frame, runes_x, row_y),
                    expected,
                    "{name}: {record:?} with an open miss: {outstanding}"
                );
                let legend = color_pixels_in_region(
                    &frame,
                    AMBER,
                    totals.x as usize..(totals.x + totals.width) as usize,
                    totals.y as usize..(totals.y + totals.height) as usize,
                ) > 0;
                assert_eq!(cracked, expected.contains(&Cracked));
                assert_eq!(
                    legend,
                    cracked || outstanding,
                    "{name}: the totals row turns amber for a cracked rune or a review"
                );
                if cracked {
                    assert_eq!(
                        codex_totals_show_legend(totals, crack_legend),
                        crack_legend,
                        "{name}"
                    );
                    // Only a crack with a pending review behind it promises
                    // one; an accuracy-only crack says so instead.
                    assert_eq!(
                        crack_legend,
                        Some(if outstanding {
                            CODEX_CRACKED_LEGEND
                        } else {
                            CODEX_SLIPPED_LEGEND
                        }),
                        "{name}: {record:?} with an open miss: {outstanding}"
                    );
                }
            }

            // A cracked rune reads differently from both a lit and an unlit one.
            let rune = |frame: &[u8], index: i32| {
                let x = (runes_x + index * 7) as usize;
                frame_region(frame, x..x + 5, row_y as usize..row_y as usize + 7)
            };
            let (cracked_frame, _) = render(gated_record(3), false);
            let (lit_frame, _) = render(gated_record(4), false);
            let (unlit_frame, _) = render(LensRecord::default(), false);
            assert_ne!(rune(&cracked_frame, 2), rune(&lit_frame, 2), "{name}");
            assert_ne!(rune(&cracked_frame, 2), rune(&unlit_frame, 2), "{name}");
            if name == "oracle" {
                maybe_write_preview("oracle-codex-mastery-cracked", &cracked_frame);
            }
        }
    }

    #[test]
    fn codex_lesson_pages_show_the_answer_and_mark_outstanding_reviews() {
        let mut cartridge = oracle_template_cartridge();
        cartridge.lessons = journal_lessons();
        cartridge.mastery = journal_mastery();
        let mut state = GameState {
            cartridge: Some(cartridge),
            ..Default::default()
        };
        let status = (44..127, 20..27);
        let answer = (
            CODEX_ANSWER_BOX.x as usize..(CODEX_ANSWER_BOX.x + CODEX_ANSWER_BOX.width) as usize,
            CODEX_ANSWER_BOX.y as usize..(CODEX_ANSWER_BOX.y + CODEX_ANSWER_BOX.height) as usize,
        );
        let rationale = (23..226, 116..139);
        let whole_rationale = (
            CODEX_RATIONALE_BOX.x as usize
                ..(CODEX_RATIONALE_BOX.x + CODEX_RATIONALE_BOX.width) as usize,
            CODEX_RATIONALE_BOX.y as usize
                ..(CODEX_RATIONALE_BOX.y + CODEX_RATIONALE_BOX.height) as usize,
        );
        let once_chose_row = (
            CODEX_TEXT_X as usize..226,
            (CODEX_RATIONALE_Y + 3 * LINE_HEIGHT) as usize
                ..(CODEX_RATIONALE_Y + 3 * LINE_HEIGHT + 7) as usize,
        );
        let render_page = |state: &GameState, page, legacy: bool, revealed: bool| {
            let mut state_frame = Framebuffer::default();
            let mut paged = GameState {
                cartridge: state.cartridge.clone(),
                codex_page: page,
                codex_revealed: revealed,
                ..Default::default()
            };
            paged.hero_style = state.hero_style;
            if legacy {
                render_codex(&mut state_frame, &paged);
            } else {
                render_oracle_codex(&mut state_frame, &paged);
            }
            state_frame.pixels
        };
        let render =
            |state: &GameState, page, legacy: bool| render_page(state, page, legacy, false);
        let count =
            |frame: &[u8], color, region: &(std::ops::Range<usize>, std::ops::Range<usize>)| {
                color_pixels_in_region(frame, color, region.0.clone(), region.1.clone())
            };

        let learned = render(&state, 1, false);
        maybe_write_preview("oracle-codex-lesson-learned", &learned);
        assert!(color_pixels_in_region(&learned, CYAN, status.0.clone(), status.1.clone()) > 0);
        assert_eq!(
            color_pixels_in_region(&learned, AMBER, status.0.clone(), status.1.clone()),
            0
        );
        assert!(color_pixels_in_region(&learned, GREEN, answer.0.clone(), answer.1.clone()) > 0);
        assert_eq!(
            render_page(&state, 1, false, true),
            learned,
            "a learned lesson has nothing to reveal"
        );
        assert!(
            count(&learned, MIST, &once_chose_row) > 0,
            "a learned lesson recalls the misconception it replaced"
        );

        // A pending review is a self-test: the answer stays sealed and the
        // player's own misconception is shown instead.
        let sealed = render(&state, 2, false);
        maybe_write_preview("oracle-codex-lesson-sealed", &sealed);
        assert!(
            color_pixels_in_region(&sealed, AMBER, status.0.clone(), status.1.clone()) > 0,
            "an outstanding lesson reads REVIEW PENDING in amber"
        );
        assert_eq!(
            count(&sealed, GREEN, &answer),
            0,
            "a pending answer is never shown before A"
        );
        assert_eq!(count(&sealed, GREEN, &whole_rationale), 0);
        assert!(
            count(&sealed, MIST, &answer) > 0,
            "the answer panel says how to reveal"
        );
        assert!(
            count(&sealed, RED, &whole_rationale) > 0,
            "the pick is shown"
        );
        assert!(
            count(&sealed, MIST, &whole_rationale) > 0,
            "and its misconception"
        );
        assert_eq!(
            count(&sealed, PARCH, &whole_rationale),
            0,
            "the answer's rationale stays sealed too"
        );

        let revealed = render_page(&state, 2, false, true);
        maybe_write_preview("oracle-codex-lesson-revealed", &revealed);
        assert!(count(&revealed, GREEN, &answer) > 0);
        assert!(count(&revealed, PARCH, &rationale) > 0);
        assert_eq!(
            count(&revealed, RED, &whole_rationale),
            0,
            "revealing shows the answer layout"
        );
        assert_eq!(count(&revealed, MIST, &once_chose_row), 0);

        let mut peeked = GameState {
            cartridge: state.cartridge.clone(),
            ..Default::default()
        };
        peeked.cartridge.as_mut().unwrap().lessons[1].peeked = true;
        let peeked_page = render_page(&peeked, 2, false, true);
        assert!(
            color_pixels_in_region(&peeked_page, AMBER, status.0.clone(), status.1.clone()) > 0
        );
        assert_ne!(
            frame_region(&peeked_page, status.0.clone(), status.1.clone()),
            frame_region(&revealed, status.0.clone(), status.1.clone()),
            "a peeked review reads PENDING PEEKED"
        );

        // A miss recorded before picks were saved still seals its answer.
        let mut unrecorded = GameState {
            cartridge: state.cartridge.clone(),
            ..Default::default()
        };
        unrecorded.cartridge.as_mut().unwrap().lessons[1].misconception = None;
        let think = render(&unrecorded, 2, false);
        maybe_write_preview("oracle-codex-lesson-sealed-no-pick", &think);
        assert_eq!(count(&think, GREEN, &answer), 0);
        assert_eq!(count(&think, RED, &whole_rationale), 0);
        let mut expected_think = Framebuffer::default();
        expected_think.text(CODEX_TEXT_X, CODEX_CHOSE_Y, CODEX_THINK_PROMPT, MIST, 1);
        let think_row = (
            CODEX_TEXT_X as usize..226,
            CODEX_CHOSE_Y as usize..(CODEX_CHOSE_Y + 7) as usize,
        );
        assert_eq!(
            count(&think, MIST, &think_row),
            count(&expected_think.pixels, MIST, &think_row),
            "without a pick the page asks the player to think first"
        );

        let legacy_lesson = render(&state, 3, false);
        maybe_write_preview("oracle-codex-lesson-no-rationale", &legacy_lesson);
        assert!(
            color_pixels_in_region(
                &legacy_lesson,
                MIST,
                rationale.0.clone(),
                rationale.1.clone()
            ) > 0
        );
        assert_eq!(
            color_pixels_in_region(&legacy_lesson, PARCH, rationale.0.clone(), rationale.1),
            0,
            "a lesson without a rationale says so instead of inventing one"
        );

        state.cartridge.as_mut().unwrap().codequest = None;
        let plain = render_page(&state, 2, true, true);
        maybe_write_preview("codex-legacy-lesson", &plain);
        assert!(color_pixels_in_region(&plain, AMBER, 5..90, 21..28) > 0);
        assert!(color_pixels_in_region(&plain, GREEN, 21..212, 76..83) > 0);
        let plain_sealed = render(&state, 2, true);
        maybe_write_preview("codex-legacy-lesson-sealed", &plain_sealed);
        assert_eq!(
            color_pixels_in_region(&plain_sealed, GREEN, 0..WIDTH, 0..HEIGHT),
            0,
            "the legacy Codex seals a pending answer too"
        );
        assert!(color_pixels_in_region(&plain_sealed, RED, 6..234, 91..141) > 0);
        assert!(color_pixels_in_region(&render(&state, 1, true), MIST, 11..215, 131..138) > 0);
        let mut plain_state = GameState {
            cartridge: state.cartridge.clone(),
            ..Default::default()
        };
        plain_state.codex_page = 0;
        let mut plain_mastery = Framebuffer::default();
        render_codex(&mut plain_mastery, &plain_state);
        maybe_write_preview("codex-legacy-mastery", &plain_mastery.pixels);
        assert_ne!(plain, plain_mastery.pixels);
    }

    #[test]
    fn codex_ui_stays_contained_disjoint_and_readable() {
        let screen = LayoutBounds {
            x: 0,
            y: 0,
            width: WIDTH as i32,
            height: HEIGHT as i32,
        };
        // Measured usable interior of the chronicle plate's archive frame.
        let archive = LayoutBounds {
            x: 62,
            y: 37,
            width: 116,
            height: 77,
        };
        let totals = inset(CODEX_TOTALS_BOX);
        let prompt = inset(CODEX_PROMPT_BOX);
        let header = inset(CODEX_LESSON_HEADER_BOX);
        let question = inset(CODEX_QUESTION_BOX);
        let answer = inset(CODEX_ANSWER_BOX);
        let rationale = inset(CODEX_RATIONALE_BOX);
        let portrait = LayoutBounds {
            x: 8,
            y: 5,
            width: HERO_PORTRAIT_SIZE as i32,
            height: HERO_PORTRAIT_SIZE as i32,
        };

        let heading = centered_text_box_bounds(CODEX_HEADING_BOX, "ORACLE CODEX", 1);
        let widest_lens = Concept::ALL
            .iter()
            .map(|concept| concept.label())
            .max_by_key(|label| label.len())
            .unwrap();
        let mut archive_children = vec![("mastery heading", heading, PARCH)];
        for row in 0..Concept::ALL.len() as i32 {
            let y = CODEX_LENS_ROW_Y + row * CODEX_LENS_ROW_PITCH;
            archive_children.push((
                "lens label",
                text_bounds(CODEX_LENS_LABEL_X, y, widest_lens, 1),
                PARCH,
            ));
            archive_children.push((
                "lens runes",
                LayoutBounds {
                    x: CODEX_LENS_RUNES_X,
                    y,
                    width: 19,
                    height: 7,
                },
                CYAN,
            ));
            archive_children.push((
                "lens pending count",
                text_bounds(CODEX_LENS_PENDING_X, y, "!99", 1),
                AMBER,
            ));
        }
        let worst_totals = "LEARNED 99  REVIEW 99";
        let lesson_counter = text_bounds(44, 7, &codex_lesson_counter(998, 999), 1);
        let lesson_lens = text_bounds(214 - 6 - text_width(widest_lens, 1), 7, widest_lens, 1);
        let lesson_runes = LayoutBounds {
            x: 214,
            y: 7,
            width: 19,
            height: 7,
        };
        let general_lens = text_bounds(214 + 19 - text_width("GENERAL", 1), 7, "GENERAL", 1);
        let lesson_status = text_bounds(44, 20, "REVIEW PENDING", 1);
        let lesson_controls = text_bounds(147, 20, "L/R:PAGE B:BACK", 1);
        let question_block = LayoutBounds {
            x: CODEX_TEXT_X,
            y: CODEX_QUESTION_Y,
            width: text_width(&"Q".repeat(QUIZ_QUESTION_COLUMNS), 1),
            height: QUIZ_QUESTION_ROWS as i32 * LINE_HEIGHT - 1,
        };
        let answer_copy = text_bounds(
            CODEX_ANSWER_TEXT_X,
            CODEX_ANSWER_Y,
            &"A".repeat(QUIZ_CHOICE_CHARS),
            1,
        );
        let answer_rune = LayoutBounds {
            x: CODEX_TEXT_X,
            y: CODEX_ANSWER_Y,
            width: 5,
            height: 7,
        };
        let rationale_heading = text_bounds(CODEX_TEXT_X, CODEX_WHY_Y, "WHY IT HOLDS", 1);
        let rationale_block = LayoutBounds {
            x: CODEX_TEXT_X,
            y: CODEX_RATIONALE_Y,
            width: text_width(&"R".repeat(RATIONALE_COLUMNS), 1),
            height: RATIONALE_ROWS as i32 * LINE_HEIGHT - 1,
        };
        let missing_rationale = text_bounds(
            CODEX_TEXT_X,
            CODEX_RATIONALE_Y,
            "NO RATIONALE WAS RECORDED",
            1,
        );
        let once_chose = LayoutBounds {
            y: CODEX_RATIONALE_Y + RATIONALE_ROWS as i32 * LINE_HEIGHT,
            height: 7,
            ..rationale_block
        };
        let reveal_prompt = text_bounds(CODEX_TEXT_X, CODEX_ANSWER_Y, CODEX_REVEAL_PROMPT, 1);
        let chose_heading = text_bounds(CODEX_TEXT_X, CODEX_CHOSE_Y, CODEX_CHOSE_HEADING, 1);
        let think_prompt = text_bounds(CODEX_TEXT_X, CODEX_CHOSE_Y, CODEX_THINK_PROMPT, 1);
        let pick_line = text_bounds(
            CODEX_TEXT_X,
            CODEX_CHOSE_Y + CODEX_CHOSE_GAP,
            &codex_pick_line(WORST_LESSON_PICK),
            1,
        );
        let misconception_block = LayoutBounds {
            x: CODEX_TEXT_X,
            y: CODEX_CHOSE_Y + 2 * CODEX_CHOSE_GAP,
            width: text_width(&"R".repeat(RATIONALE_COLUMNS), 1),
            height: RATIONALE_ROWS as i32 * LINE_HEIGHT - 1,
        };
        let menu_option =
            centered_text_box_bounds(GATEWAY_MENU_OPTION_TEXT_BOXES[1], "OPEN THE CODEX", 1);
        let menu_subtitle =
            centered_text_box_bounds(GATEWAY_MENU_SUBTITLE_BOX, "LESSONS 99  REVIEW 99", 1);

        let mut contained = archive_children
            .iter()
            .map(|(name, child, _)| (*name, archive, *child))
            .collect::<Vec<_>>();
        contained.extend([
            (
                "totals",
                totals,
                centered_text_box_bounds(CODEX_TOTALS_BOX, worst_totals, 1),
            ),
            (
                "empty journal",
                totals,
                centered_text_box_bounds(CODEX_TOTALS_BOX, "NO LESSONS YET", 1),
            ),
            (
                "cracked legend",
                totals,
                centered_text_box_bounds(CODEX_TOTALS_BOX, CODEX_CRACKED_LEGEND, 1),
            ),
            (
                "accuracy legend",
                totals,
                centered_text_box_bounds(CODEX_TOTALS_BOX, CODEX_SLIPPED_LEGEND, 1),
            ),
            (
                "reading controls",
                prompt,
                centered_text_box_bounds(CODEX_PROMPT_BOX, "L/R:READ LESSONS  B:BACK", 1),
            ),
            (
                "empty guidance",
                prompt,
                centered_text_box_bounds(CODEX_PROMPT_BOX, "ANSWER A TRIAL  B:BACK", 1),
            ),
            ("lesson counter", header, lesson_counter),
            ("lesson lens", header, lesson_lens),
            ("lesson lens runes", header, lesson_runes),
            ("lesson without lens", header, general_lens),
            ("lesson status", header, lesson_status),
            ("lesson controls", header, lesson_controls),
            ("lesson question", question, question_block),
            ("lesson answer", answer, answer_copy),
            ("lesson answer rune", answer, answer_rune),
            ("rationale heading", rationale, rationale_heading),
            ("rationale", rationale, rationale_block),
            ("missing rationale", rationale, missing_rationale),
            ("once chose", rationale, once_chose),
            ("reveal prompt", answer, reveal_prompt),
            ("you chose heading", rationale, chose_heading),
            ("think prompt", rationale, think_prompt),
            ("worst pick", rationale, pick_line),
            ("worst misconception", rationale, misconception_block),
            (
                "menu codex option",
                ui_box_bounds(GATEWAY_MENU_OPTION_TEXT_BOXES[1]),
                menu_option,
            ),
            (
                "menu journal summary",
                ui_box_bounds(GATEWAY_MENU_SUBTITLE_BOX),
                menu_subtitle,
            ),
        ]);
        for (name, container, child) in contained {
            assert!(
                bounds_contains(container, child),
                "{name} {child:?} exceeds its container {container:?}"
            );
            assert!(
                bounds_contains(screen, child),
                "{name} {child:?} exceeds the native frame"
            );
        }
        assert!(horizontal_centers_align(
            ui_box_bounds(CODEX_HEADING_BOX),
            heading
        ));

        for (index, (left_name, left, _)) in archive_children.iter().enumerate() {
            for (right_name, right, _) in &archive_children[index + 1..] {
                assert!(
                    bounds_are_disjoint(*left, *right),
                    "{left_name} {left:?} overlaps {right_name} {right:?}"
                );
            }
        }
        for (name, left, right) in [
            (
                "archive and totals",
                archive,
                ui_box_bounds(CODEX_TOTALS_BOX),
            ),
            (
                "totals and prompt",
                ui_box_bounds(CODEX_TOTALS_BOX),
                ui_box_bounds(CODEX_PROMPT_BOX),
            ),
            (
                "portrait and header",
                portrait,
                ui_box_bounds(CODEX_LESSON_HEADER_BOX),
            ),
            ("counter and lens", lesson_counter, lesson_lens),
            ("counter and lensless label", lesson_counter, general_lens),
            ("lens and runes", lesson_lens, lesson_runes),
            ("counter and status", lesson_counter, lesson_status),
            ("status and controls", lesson_status, lesson_controls),
            (
                "header and question",
                ui_box_bounds(CODEX_LESSON_HEADER_BOX),
                ui_box_bounds(CODEX_QUESTION_BOX),
            ),
            (
                "question and answer",
                ui_box_bounds(CODEX_QUESTION_BOX),
                answer,
            ),
            ("answer rune and answer", answer_rune, answer_copy),
            (
                "answer and rationale",
                ui_box_bounds(CODEX_ANSWER_BOX),
                ui_box_bounds(CODEX_RATIONALE_BOX),
            ),
            (
                "rationale heading and copy",
                rationale_heading,
                rationale_block,
            ),
            ("rationale and once chose", rationale_block, once_chose),
            ("you chose heading and pick", chose_heading, pick_line),
            ("think prompt and pick", think_prompt, pick_line),
            ("pick and misconception", pick_line, misconception_block),
        ] {
            assert!(
                bounds_are_disjoint(left, right),
                "{name} overlap: {left:?} and {right:?}"
            );
        }

        // Copy drawn straight onto a plate must clear every ornament: test it
        // against the brightest plate pixel inside its own glyph cells.
        let mut plate_copy = archive_children
            .iter()
            .map(|(name, bounds, color)| (*name, ORACLE_CHRONICLE, *bounds, *color))
            .collect::<Vec<_>>();
        plate_copy.push(("unlit lens", ORACLE_CHRONICLE, archive_children[1].1, MIST));
        for (name, color) in [("amber rune", AMBER), ("magenta rune", MAGENTA)] {
            plate_copy.push((name, ORACLE_CHRONICLE, archive_children[2].1, color));
        }
        for (name, plate, bounds, color) in plate_copy {
            let fill = brightest_plate_color(plate, bounds);
            let ratio = contrast_ratio(color, fill);
            assert!(
                ratio >= 4.5,
                "{name} contrast {ratio:.2}:1 against plate fill {:?} is below 4.5:1",
                (fill.0, fill.1, fill.2)
            );
        }
        for (name, foreground, background) in [
            ("legacy counter", SKY, INK),
            ("legacy lens", PARCH, INK),
            ("legacy status", AMBER, NAVY),
            ("legacy learned", CYAN, NAVY),
            ("legacy unlit lens", MIST, NAVY),
            ("legacy pending", AMBER, NAVY),
            ("lesson answer panel", GREEN, VOID),
            ("lesson question panel", PARCH, VOID),
            ("rationale heading panel", CYAN_DIM, VOID),
            ("lesson status panel", AMBER, VOID),
            ("cracked legend panel", AMBER, VOID),
            ("lesson controls panel", MIST, VOID),
            ("sealed pick panel", RED, VOID),
            ("sealed misconception panel", MIST, VOID),
            ("legacy once chose", MIST, INK),
            ("legacy answer", GREEN, INK),
            ("legacy answer marker", GOLD, INK),
            ("legacy empty journal", GOLD, NAVY),
            ("legacy question", PARCH, INK),
            ("legacy rationale heading", SKY, INK),
        ] {
            let ratio = contrast_ratio(foreground, background);
            assert!(ratio >= 4.5, "{name} contrast {ratio:.2}:1 is below 4.5:1");
        }
    }

    #[test]
    fn codex_legacy_layout_keeps_worst_case_copy_inside_its_panels() {
        let question_box = LayoutBounds {
            x: 6,
            y: 32,
            width: 228,
            height: 36,
        };
        let rationale_box = LayoutBounds {
            x: 6,
            y: 91,
            width: 228,
            height: 50,
        };
        let mastery_box = LayoutBounds {
            x: 31,
            y: 25,
            width: 178,
            height: 80,
        };
        for (name, container, child) in [
            (
                "question",
                question_box,
                LayoutBounds {
                    x: 11,
                    y: 35,
                    width: text_width(&"Q".repeat(QUIZ_QUESTION_COLUMNS), 1),
                    height: QUIZ_QUESTION_ROWS as i32 * LINE_HEIGHT - 1,
                },
            ),
            (
                "rationale heading",
                rationale_box,
                text_bounds(11, 95, "WHY IT HOLDS", 1),
            ),
            (
                "rationale",
                rationale_box,
                LayoutBounds {
                    x: 11,
                    y: 107,
                    width: text_width(&"R".repeat(RATIONALE_COLUMNS), 1),
                    height: RATIONALE_ROWS as i32 * LINE_HEIGHT - 1,
                },
            ),
            (
                "once chose",
                rationale_box,
                text_bounds(11, 107 + 3 * LINE_HEIGHT, &"O".repeat(RATIONALE_COLUMNS), 1),
            ),
            (
                "reveal prompt",
                LayoutBounds {
                    x: 6,
                    y: 73,
                    width: 228,
                    height: 13,
                },
                text_bounds(9, 76, CODEX_REVEAL_PROMPT, 1),
            ),
            (
                "you chose heading",
                rationale_box,
                text_bounds(11, 95, CODEX_CHOSE_HEADING, 1),
            ),
            (
                "worst pick",
                rationale_box,
                text_bounds(11, 105, &codex_pick_line(WORST_LESSON_PICK), 1),
            ),
            (
                "worst misconception",
                rationale_box,
                LayoutBounds {
                    x: 11,
                    y: 115,
                    width: text_width(&"R".repeat(RATIONALE_COLUMNS), 1),
                    height: RATIONALE_ROWS as i32 * LINE_HEIGHT - 1,
                },
            ),
            (
                "first lens",
                mastery_box,
                text_bounds(42, 32, "INVARIANTS", 1),
            ),
            (
                "last pending count",
                mastery_box,
                text_bounds(156, 32 + 4 * 14, "!99", 1),
            ),
        ] {
            assert!(
                bounds_contains(container, child),
                "{name} {child:?} exceeds {container:?}"
            );
        }
        assert!(bounds_are_disjoint(
            text_bounds(5, 4, &codex_lesson_counter(998, 999), 1),
            text_bounds(214 - 6 - text_width("INVARIANTS", 1), 4, "INVARIANTS", 1),
        ));
        assert!(bounds_are_disjoint(
            centered_text_bounds(116, "LEARNED 99  REVIEW 99", 1),
            text_bounds(5, 151, "L/R:PAGE", 1),
        ));
    }

    #[test]
    fn codex_fixture_copy_is_worst_case_for_the_lesson_panels() {
        assert_eq!(
            wrap_text(WORST_LESSON_QUESTION, QUIZ_QUESTION_COLUMNS).len(),
            QUIZ_QUESTION_ROWS
        );
        let rationale = wrap_text(WORST_LESSON_RATIONALE, RATIONALE_COLUMNS);
        assert_eq!(rationale.len(), RATIONALE_ROWS);
        assert_eq!(rationale[0].chars().count(), RATIONALE_COLUMNS);
        assert!(crate::learning::rationale_fits(WORST_LESSON_RATIONALE));
        assert_eq!(WORST_LESSON_PICK.chars().count(), QUIZ_CHOICE_CHARS);
        let once = codex_once_chose_line(WORST_LESSON_PICK);
        assert!(once.chars().count() <= RATIONALE_COLUMNS, "{once}");
        assert!(once.ends_with("..."), "a cut pick is marked as cut: {once}");
        assert_eq!(codex_once_chose_line("THE SHELL"), "ONCE CHOSE: THE SHELL");
        assert_eq!(
            wrap_text(WORST_LESSON_MISCONCEPTION, RATIONALE_COLUMNS).len(),
            RATIONALE_ROWS
        );
        assert!(crate::learning::rationale_fits(WORST_LESSON_MISCONCEPTION));
        for lesson in journal_lessons() {
            assert!(
                lesson.answer.chars().count() <= QUIZ_CHOICE_CHARS,
                "lesson answers are committed quiz choices and share their limit"
            );
            if let Some((pick, _)) = &lesson.misconception {
                assert!(pick.chars().count() <= QUIZ_CHOICE_CHARS);
            }
        }
    }
}
