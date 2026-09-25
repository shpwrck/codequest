use super::*;

/// The composed lesson panel that replaces the trial plate's four choice
/// frames after a commitment. It spans the frames plus the free margin beside
/// them so a full rationale row keeps visible padding inside the border.
pub(super) const TRIAL_LESSON_BOX: UiBox = UiBox {
    x: 20,
    y: 69,
    width: 210,
    height: 89,
};
/// The legacy quiz's lesson panel over its choice rows.
pub(super) const QUIZ_LESSON_BOX: UiBox = UiBox {
    x: 5,
    y: 68,
    width: 230,
    height: 80,
};
/// Top of the legacy lesson card's INK footer strip, just below the panel.
pub(super) const QUIZ_LESSON_FOOTER_STRIP_Y: i32 = QUIZ_LESSON_BOX.y + QUIZ_LESSON_BOX.height;
/// Extra pixels between the misconception block and the answer block.
pub(super) const LESSON_BLOCK_GAP: i32 = 4;

/// True once the lesson card's input hold has elapsed and A or Start will
/// continue.
pub(super) fn lesson_is_live(run: &QuizRun) -> bool {
    matches!(run.feedback, Some((_, 0)))
}

pub(super) const LESSON_CONTINUE: &str = "A:CONTINUE";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LessonTone {
    /// The player's wrong pick: the misconception it reveals.
    Misconception,
    MisconceptionWhy,
    /// The correct answer and why it holds.
    Answer,
    AnswerWhy,
}

impl LessonTone {
    pub(super) fn color(self) -> Color {
        match self {
            Self::Misconception => RED,
            Self::MisconceptionWhy => MIST,
            Self::Answer => GREEN,
            Self::AnswerWhy => PARCH,
        }
    }
}

/// One positioned line of lesson copy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LessonLine {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) text: String,
    pub(super) tone: LessonTone,
}

/// Composes the lesson card copy inside `area`: for a miss, the player's pick
/// with its rationale and then the correct answer with its rationale; for a
/// success, the answer with its rationale. Choice lines carry a `-` or `+`
/// marker as well as color, so the verdict never depends on hue alone. The
/// fixed rationale column is centered horizontally and the whole composition
/// vertically; legacy questions without rationales show only choice lines.
pub(super) fn lesson_lines(
    question: &QuizQuestion,
    picked: usize,
    correct: bool,
    area: UiBox,
) -> Vec<LessonLine> {
    let mut blocks: Vec<Vec<(String, LessonTone)>> = Vec::new();
    let mut block = |marker: &str, index: usize, tone: LessonTone, why: LessonTone| {
        let choice = question.choices.get(index).map_or("", String::as_str);
        let mut lines = vec![(
            format!("{marker} {}", truncate(choice, QUIZ_CHOICE_CHARS)),
            tone,
        )];
        if let Some(rationale) = choice_rationale(question, index) {
            lines.extend(
                wrap_text(rationale, RATIONALE_COLUMNS)
                    .into_iter()
                    .take(RATIONALE_ROWS)
                    .map(|line| (line, why)),
            );
        }
        blocks.push(lines);
    };
    if !correct {
        block(
            "-",
            picked,
            LessonTone::Misconception,
            LessonTone::MisconceptionWhy,
        );
    }
    block(
        "+",
        question.answer,
        LessonTone::Answer,
        LessonTone::AnswerWhy,
    );

    let line_count = blocks.iter().map(Vec::len).sum::<usize>() as i32;
    let height = line_count * LINE_HEIGHT - 1 + (blocks.len() as i32 - 1) * LESSON_BLOCK_GAP;
    let column_width = text_width(&"W".repeat(RATIONALE_COLUMNS), 1);
    let x = area.x + (area.width - column_width) / 2;
    let mut y = area.y + (area.height - height) / 2;
    let mut positioned = Vec::new();
    for block in blocks {
        for (text, tone) in block {
            positioned.push(LessonLine { x, y, text, tone });
            y += LINE_HEIGHT;
        }
        y += LESSON_BLOCK_GAP;
    }
    positioned
}

pub(super) fn draw_lesson_lines(frame: &mut Framebuffer, lines: &[LessonLine]) {
    for line in lines {
        frame.text(line.x, line.y, &line.text, line.tone.color(), 1);
    }
}

/// The trial lesson panel's copy region above its footer row.
pub(super) fn trial_lesson_copy_box() -> UiBox {
    UiBox {
        x: TRIAL_LESSON_BOX.x + 1,
        y: TRIAL_LESSON_BOX.y + 2,
        width: TRIAL_LESSON_BOX.width - 2,
        height: TRIAL_LESSON_BOX.height - 17,
    }
}

/// The trial lesson panel's footer row: the lens and its mastery runes on the
/// left, and the continue prompt on the right once input is live.
pub(super) fn trial_lesson_footer_y() -> i32 {
    TRIAL_LESSON_BOX.y + TRIAL_LESSON_BOX.height - 11
}

/// The legacy lesson panel's copy region inside its border.
pub(super) fn quiz_lesson_copy_box() -> UiBox {
    UiBox {
        x: QUIZ_LESSON_BOX.x + 1,
        y: QUIZ_LESSON_BOX.y + 1,
        width: QUIZ_LESSON_BOX.width - 2,
        height: QUIZ_LESSON_BOX.height - 2,
    }
}

/// The committed result's lesson copy, when the run is showing one.
pub(super) fn current_lesson(state: &GameState, area: UiBox) -> Option<Vec<LessonLine>> {
    let run = state.quiz.as_ref()?;
    let (correct, _) = run.feedback?;
    let question = state.cartridge.as_ref()?.questions.get(run.question)?;
    let picked = run.source_choice(run.selected, question.choices.len());
    Some(lesson_lines(question, picked, correct, area))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lesson_panel_copy_stays_contained_disjoint_and_readable() {
        let question = worst_case_lesson_question();
        assert!(question.rationales.iter().all(|rationale| wrap_text(
            rationale,
            RATIONALE_COLUMNS
        )
        .len()
            == RATIONALE_ROWS
            && learning::rationale_fits(rationale)));
        let panel = ui_box_bounds(TRIAL_LESSON_BOX);
        let interior = LayoutBounds {
            x: panel.x + 1,
            y: panel.y + 1,
            width: panel.width - 2,
            height: panel.height - 2,
        };
        let screen = LayoutBounds {
            x: 0,
            y: 0,
            width: WIDTH as i32,
            height: HEIGHT as i32,
        };
        let question_copy = text_bounds(
            28,
            35 + (QUIZ_QUESTION_ROWS as i32 - 1) * LINE_HEIGHT,
            &"Q".repeat(QUIZ_QUESTION_COLUMNS),
            1,
        );
        assert!(bounds_contains(screen, panel));
        assert!(bounds_are_disjoint(panel, question_copy));

        for (name, correct, expected_lines) in [("missed", false, 8), ("correct", true, 4)] {
            let lines = lesson_lines(&question, 1, correct, trial_lesson_copy_box());
            assert_eq!(lines.len(), expected_lines, "{name}");
            let column_x = lines[0].x;
            let column_end = column_x + text_width(&"W".repeat(RATIONALE_COLUMNS), 1);
            let footer_y = trial_lesson_footer_y();
            let label = text_bounds(column_x, footer_y, Concept::Invariant.label(), 1);
            let runes = LayoutBounds {
                x: label.x + label.width + 4,
                y: footer_y,
                width: 19,
                height: 7,
            };
            let prompt = text_bounds(
                column_end - text_width(LESSON_CONTINUE, 1),
                footer_y,
                LESSON_CONTINUE,
                1,
            );
            let rule = LayoutBounds {
                x: column_x,
                y: footer_y - 3,
                width: column_end - column_x,
                height: 1,
            };
            let mut children: Vec<(String, LayoutBounds)> = lines
                .iter()
                .map(|line| {
                    (
                        line.text.clone(),
                        text_bounds(line.x, line.y, &line.text, 1),
                    )
                })
                .collect();
            children.extend([
                ("lens label".to_string(), label),
                ("lens runes".to_string(), runes),
                ("continue prompt".to_string(), prompt),
                ("footer rule".to_string(), rule),
            ]);
            // Every retry note fits between the widest lens's runes and the
            // continue prompt with at least 4px on each side.
            // The notes are alternatives, so only the widest joins the
            // pairwise sibling check.
            for note in [
                RetryNote::In(0),
                RetryNote::In(12),
                RetryNote::NextRun,
                RetryNote::In(RETRY_GAP),
            ] {
                let text = note.label();
                let bounds = text_bounds(
                    trial_retry_note_x(column_x, column_end, Some(Concept::Invariant), &text),
                    footer_y,
                    &text,
                    1,
                );
                assert!(
                    bounds.x - (runes.x + runes.width) >= 4
                        && prompt.x - (bounds.x + bounds.width) >= 4,
                    "{name} {text} {bounds:?} crowds the lens runes {runes:?} or {prompt:?}"
                );
                assert!(bounds_contains(interior, bounds), "{name} {text}");
                if note == RetryNote::In(RETRY_GAP) {
                    assert_eq!(text, "BACK IN 3");
                    children.push((format!("retry note {text}"), bounds));
                }
            }
            // Without a lens the note still clears the prompt.
            let lensless = text_bounds(
                trial_retry_note_x(column_x, column_end, None, "BACK IN 9"),
                footer_y,
                "BACK IN 9",
                1,
            );
            assert!(bounds_contains(interior, lensless));
            assert!(prompt.x - (lensless.x + lensless.width) >= 4);
            for (child_name, child) in &children {
                assert!(
                    bounds_contains(interior, *child),
                    "{name} {child_name} {child:?} exceeds the lesson panel {interior:?}"
                );
            }
            for (index, (left_name, left)) in children.iter().enumerate() {
                for (right_name, right) in &children[index + 1..] {
                    assert!(
                        bounds_are_disjoint(*left, *right),
                        "{name} {left_name} overlaps {right_name}"
                    );
                }
            }
            for line in &lines {
                assert!(
                    bounds_contains(
                        ui_box_bounds(trial_lesson_copy_box()),
                        text_bounds(line.x, line.y, &line.text, 1)
                    ),
                    "{name} line `{}` leaves the copy region",
                    line.text
                );
            }
            assert_eq!(lines.last().unwrap().tone, LessonTone::AnswerWhy);
            assert!(lines[0].text.starts_with(if correct { "+ " } else { "- " }));
        }

        let legacy_panel = ui_box_bounds(QUIZ_LESSON_BOX);
        let legacy_interior = LayoutBounds {
            x: legacy_panel.x + 1,
            y: legacy_panel.y + 1,
            width: legacy_panel.width - 2,
            height: legacy_panel.height - 2,
        };
        let legacy_lines = lesson_lines(&question, 1, false, quiz_lesson_copy_box());
        for (index, line) in legacy_lines.iter().enumerate() {
            let bounds = text_bounds(line.x, line.y, &line.text, 1);
            assert!(
                bounds_contains(legacy_interior, bounds),
                "legacy {}",
                line.text
            );
            for other in &legacy_lines[index + 1..] {
                assert!(bounds_are_disjoint(
                    bounds,
                    text_bounds(other.x, other.y, &other.text, 1)
                ));
            }
        }
        let legacy_banner = text_bounds(5, 151, "INVARIANTS RUNE III", 1);
        let legacy_prompt = text_bounds(
            235 - text_width(LESSON_CONTINUE, 1),
            151,
            LESSON_CONTINUE,
            1,
        );
        assert!(bounds_are_disjoint(legacy_banner, legacy_prompt));
        assert!(bounds_are_disjoint(legacy_panel, legacy_banner));
        // The legacy footer adds the retry note only where it keeps a glyph
        // cell from both the same-colored ward banner and A:CONTINUE; the
        // widest banners drop it.
        for banner in [
            "WARD STRAINED",
            "WARD FRACTURES",
            "WARD BROKEN",
            "INVARIANTS RUNE III",
        ] {
            let banner_bounds = text_bounds(5, 151, banner, 1);
            for note in ["BACK IN 3", "UP NEXT", "NEXT RUN"] {
                if let Some(x) = quiz_retry_note_x(banner, note) {
                    let note_bounds = text_bounds(x, 151, note, 1);
                    assert!(note_bounds.x - (banner_bounds.x + banner_bounds.width) >= 6);
                    assert!(legacy_prompt.x - (note_bounds.x + note_bounds.width) >= 6);
                }
            }
        }
        // Every ward banner a miss can show keeps its note.
        for (banner, note) in [
            ("WARD STRAINED", "BACK IN 3"),
            ("WARD FRACTURES", "BACK IN 3"),
            ("WARD BROKEN", "NEXT RUN"),
        ] {
            assert!(quiz_retry_note_x(banner, note).is_some(), "{banner} {note}");
        }
        assert!(quiz_retry_note_x("INVARIANTS RUNE III", "BACK IN 3").is_none());
        let ratio = contrast_ratio(AMBER, NAVY);
        assert!(ratio >= 4.5, "legacy retry note contrast {ratio:.2}:1");
        assert!(bounds_contains(screen, legacy_prompt));
        let legacy_leave = text_bounds(
            235 - text_width("B AGAIN:LEAVE", 1),
            151,
            "B AGAIN:LEAVE",
            1,
        );
        assert!(bounds_are_disjoint(
            text_bounds(5, 151, "A:ANSWER", 1),
            legacy_leave
        ));
        assert!(bounds_contains(screen, legacy_leave));

        for tone in [
            LessonTone::Misconception,
            LessonTone::MisconceptionWhy,
            LessonTone::Answer,
            LessonTone::AnswerWhy,
        ] {
            let ratio = contrast_ratio(tone.color(), VOID);
            assert!(
                ratio >= 4.5,
                "{tone:?} contrast {ratio:.2}:1 on the lesson panel"
            );
        }
        for (name, color) in [
            ("lens", CYAN_DIM),
            ("continue", CYAN),
            ("retry note", AMBER),
        ] {
            let ratio = contrast_ratio(color, VOID);
            assert!(
                ratio >= 4.5,
                "{name} contrast {ratio:.2}:1 on the lesson panel"
            );
        }
    }

    #[test]
    fn the_lesson_footer_meter_shows_a_cracked_rune_for_an_open_miss() {
        let footer_x = |state: &GameState| {
            current_lesson(state, trial_lesson_copy_box()).unwrap()[0].x
                + text_width(Concept::Responsibility.label(), 1)
                + 4
        };
        let mut state = lesson_state(lesson_question(), false, true);
        let footer_y = trial_lesson_footer_y();
        {
            let cartridge = state.cartridge.as_mut().unwrap();
            cartridge.mastery.insert(
                Concept::Responsibility,
                LensRecord {
                    first_try: 5,
                    ..LensRecord::default()
                },
            );
            let answer = lesson_question().answer;
            record_lesson(&mut cartridge.lessons, &lesson_question(), true, answer);
        }
        let mut frame = Framebuffer::default();
        render_oracle_trial(&mut frame, &state);
        let x = footer_x(&state);
        assert_eq!(
            meter_styles(&frame.pixels, x, footer_y),
            [RuneStyle::Lit; 3]
        );

        record_lesson(
            &mut state.cartridge.as_mut().unwrap().lessons,
            &lesson_question(),
            false,
            (lesson_question().answer + 1) % 4,
        );
        let mut frame = Framebuffer::default();
        render_oracle_trial(&mut frame, &state);
        maybe_write_preview("oracle-lesson-cracked-rune", &frame.pixels);
        assert_eq!(
            meter_styles(&frame.pixels, x, footer_y),
            [RuneStyle::Lit, RuneStyle::Lit, RuneStyle::Cracked],
            "the miss just journaled cracks rune III on the footer"
        );
    }
}
