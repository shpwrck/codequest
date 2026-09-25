use super::*;

pub(super) mod aftermath;
pub(super) mod ascension;
pub(super) mod atelier;
pub(super) mod codex;
pub(super) mod datafall;
pub(super) mod menu;
pub(super) mod opening;
pub(super) mod quest;
pub(super) mod sprites;
pub(super) mod trial;
pub(super) mod widgets;

pub(super) fn render(mut frame: ResMut<Framebuffer>, state: Res<GameState>) {
    draw_screen(&mut frame, &state);
}

pub(super) fn draw_screen(frame: &mut Framebuffer, state: &GameState) {
    match state.screen {
        Screen::Off => frame.clear(INK),
        Screen::Boot => render_boot(frame, state),
        Screen::Copyright => render_copyright(frame, state),
        Screen::OpeningFanfare => render_opening_fanfare(frame, state),
        Screen::Title => render_title(frame, state),
        Screen::QuizMenu => render_quiz_menu(frame, state),
        Screen::CharacterCreation => render_character_creation(frame, state),
        Screen::Oracle => render_oracle(frame, state),
        Screen::Quiz => render_quiz(frame, state),
        Screen::LevelUp => render_level_up(frame, state),
        Screen::GameOver => render_game_over(frame, state),
        Screen::Codex => render_codex(frame, state),
        Screen::QuestSelect => render_quest_select(frame, state),
        Screen::Battle => render_battle(frame, state),
        Screen::Victory => render_result(frame, true),
        Screen::Defeat => render_result(frame, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_live_ui_stays_contained_disjoint_and_readable() {
        let screen = LayoutBounds {
            x: 0,
            y: 0,
            width: WIDTH as i32,
            height: HEIGHT as i32,
        };
        let chronicle_header = ui_box_bounds(CHRONICLE_HEADER_BOX);
        let chronicle_title = ui_box_bounds(CHRONICLE_TITLE_BOX);
        let chronicle_copyright = ui_box_bounds(CHRONICLE_COPYRIGHT_BOX);
        let chronicle_authors_label = ui_box_bounds(CHRONICLE_AUTHORS_LABEL_BOX);
        let chronicle_authors = ui_box_bounds(CHRONICLE_AUTHORS_BOX);
        let chronicle_footer = LayoutBounds {
            x: 42,
            y: 122,
            width: 156,
            height: 17,
        };
        let chronicle_skip = LayoutBounds {
            x: 74,
            y: 141,
            width: 92,
            height: 16,
        };
        let title_top = ui_box_bounds(GATEWAY_TITLE_TOP_BOX);
        let title_bottom = ui_box_bounds(GATEWAY_TITLE_BOTTOM_BOX);
        let title_prompt = ui_box_bounds(GATEWAY_PROMPT_BOX);
        let title_signature = ui_box_bounds(GATEWAY_SIGNATURE_BOX);
        let menu_option_text = ui_box_bounds(GATEWAY_MENU_OPTION_TEXT_BOXES[1]);
        let menu_heading = ui_box_bounds(GATEWAY_MENU_HEADING_BOX);
        let menu_subtitle = ui_box_bounds(GATEWAY_MENU_SUBTITLE_BOX);
        let atelier_label = ui_box_bounds(atelier_label_box(ATELIER_ROW_BOXES[1]));
        let atelier_value = ui_box_bounds(atelier_value_box(ATELIER_ROW_BOXES[1]));
        let atelier_bind_text = ui_box_bounds(ATELIER_BIND_TEXT_BOX);
        let quiz_question = LayoutBounds {
            x: 20,
            y: 30,
            width: 200,
            height: 36,
        };
        let quiz_choice = LayoutBounds {
            x: 23,
            y: 69,
            width: 204,
            height: 20,
        };
        let aftermath_panel = ui_box_bounds(AFTERMATH_CONTENT_BOX);
        let opening_skip = LayoutBounds {
            x: 164,
            y: 149,
            width: 76,
            height: 11,
        };
        let menu_footer = ui_box_bounds(MENU_FOOTER_BOX);
        let atelier_header = ui_box_bounds(ATELIER_HEADER_BOX);
        let full_header = LayoutBounds {
            x: 0,
            y: 0,
            width: 240,
            height: 15,
        };
        let full_footer = LayoutBounds {
            x: 0,
            y: 143,
            width: 240,
            height: 17,
        };
        let ascension_title = ui_box_bounds(ASCENSION_TITLE_BOX);
        let trial_header = LayoutBounds {
            x: 37,
            y: 2,
            width: 203,
            height: 28,
        };
        let trial_ward = LayoutBounds {
            x: TRIAL_WARD_X,
            y: 7,
            width: 46,
            height: 7,
        };
        let trial_streak = text_bounds(TRIAL_STREAK_X, 7, "X3", 1);
        let trial_score_runes = LayoutBounds {
            x: TRIAL_SCORE_RUNES_X,
            y: 7,
            width: 19,
            height: 7,
        };
        let trial_score = text_bounds(TRIAL_SCORE_X, 7, "9999", 1);
        let ascension_level = ui_box_bounds(ASCENSION_LEVEL_BOX);
        let ascension_batch = ui_box_bounds(ASCENSION_BATCH_BOX);

        for (name, container, child) in [
            (
                "chronicle heading",
                chronicle_header,
                centered_compact_text_box_bounds(CHRONICLE_HEADER_BOX, "REPOSITORY CHRONICLE"),
            ),
            (
                "chronicle title first line",
                chronicle_title,
                compact_text_bounds(62, 49, "T".repeat(23).as_str()),
            ),
            (
                "chronicle title second line",
                chronicle_title,
                compact_text_bounds(62, 57, "T".repeat(23).as_str()),
            ),
            (
                "chronicle copyright first line",
                chronicle_copyright,
                compact_text_bounds(62, 67, "C".repeat(23).as_str()),
            ),
            (
                "chronicle copyright second line",
                chronicle_copyright,
                compact_text_bounds(62, 75, "C".repeat(23).as_str()),
            ),
            (
                "chronicle author heading",
                chronicle_authors_label,
                centered_compact_text_box_bounds(CHRONICLE_AUTHORS_LABEL_BOX, "COMMIT AUTHORS"),
            ),
            (
                "chronicle third author",
                chronicle_authors,
                compact_text_bounds(62, 110, "A".repeat(23).as_str()),
            ),
            (
                "chronicle history",
                chronicle_footer,
                centered_text_bounds(126, "ARCHIVE 1234 > 56789012", 1),
            ),
            (
                "chronicle skip",
                chronicle_skip,
                centered_text_bounds(145, "A / START:SKIP", 1),
            ),
            (
                "title first line",
                title_top,
                centered_text_box_bounds(GATEWAY_TITLE_TOP_BOX, "CODE QUEST", 2),
            ),
            (
                "title second line",
                title_bottom,
                centered_text_box_bounds(GATEWAY_TITLE_BOTTOM_BOX, "ADVANCE", 1),
            ),
            (
                "title prompt",
                title_prompt,
                centered_text_box_bounds(GATEWAY_PROMPT_BOX, "PRESS START", 1),
            ),
            (
                "title signature",
                title_signature,
                centered_text_box_bounds(GATEWAY_SIGNATURE_BOX, "REPOSITORY ORACLE", 1),
            ),
            (
                "awakening skip",
                opening_skip,
                text_bounds(166, 151, "A/START:SKIP", 1),
            ),
            (
                "menu option",
                menu_option_text,
                centered_text_box_bounds(GATEWAY_MENU_OPTION_TEXT_BOXES[1], "RETURN TO TITLE", 1),
            ),
            (
                "menu heading",
                menu_heading,
                centered_text_box_bounds(GATEWAY_MENU_HEADING_BOX, "CHOOSE YOUR PATH", 1),
            ),
            (
                "menu subtitle",
                menu_subtitle,
                centered_text_box_bounds(GATEWAY_MENU_SUBTITLE_BOX, "THE BOND BEGINS HERE", 1),
            ),
            (
                "menu controls",
                menu_footer,
                centered_text_box_bounds(MENU_FOOTER_BOX, "D-PAD  A:CHOOSE  B:BACK", 1),
            ),
            (
                "atelier heading",
                atelier_header,
                centered_compact_text_box_bounds(ATELIER_HEADER_BOX, "BIND YOUR CODE-SEER"),
            ),
            (
                "atelier row label",
                atelier_label,
                centered_text_box_bounds(atelier_label_box(ATELIER_ROW_BOXES[1]), "PATH", 1),
            ),
            (
                "atelier longest value",
                atelier_value,
                centered_compact_text_box_bounds(
                    atelier_value_box(ATELIER_ROW_BOXES[1]),
                    "<MERGE PALADIN>",
                ),
            ),
            (
                "atelier bind action",
                atelier_bind_text,
                centered_text_box_bounds(ATELIER_BIND_TEXT_BOX, "BIND", 1),
            ),
            (
                "atelier retry status",
                full_footer,
                text_bounds(5, 148, "VISION CLOUDY - RETRYING", 1),
            ),
            (
                "atelier controls",
                full_footer,
                text_bounds(174, 148, "START:BIND", 1),
            ),
            (
                "sanctum status",
                full_header,
                text_bounds(152, 5, "CLAUDE:SCRYING", 1),
            ),
            (
                "sanctum controls",
                full_footer,
                centered_text_bounds(149, "L/R:MOVE  B:LEAVE", 1),
            ),
            (
                "quiz longest question line",
                quiz_question,
                text_bounds(28, 35, &"Q".repeat(QUIZ_QUESTION_COLUMNS), 1),
            ),
            (
                "trial number",
                trial_header,
                text_bounds(44, 7, "TRIAL 99", 1),
            ),
            (
                "trial batch progress",
                trial_header,
                text_bounds(44, 7, "RETRY 12/13", 1),
            ),
            (
                "trial lens banner",
                trial_header,
                text_bounds(
                    trial_banner_x("INVARIANTS RUNE III"),
                    20,
                    "INVARIANTS RUNE III",
                    1,
                ),
            ),
            (
                "trial insight banner",
                trial_header,
                text_bounds(
                    trial_banner_x("INSIGHT III RISES"),
                    20,
                    "INSIGHT III RISES",
                    1,
                ),
            ),
            ("trial ward", trial_header, trial_ward),
            ("trial streak multiplier", trial_header, trial_streak),
            ("trial score runes", trial_header, trial_score_runes),
            ("trial score", trial_header, trial_score),
            (
                "trial tier",
                trial_header,
                text_bounds(44, 20, "ORACLE-BOUND", 1),
            ),
            (
                "trial controls",
                trial_header,
                text_bounds(126, 20, "A:ANSWER B:LEAVE", 1),
            ),
            (
                "quiz longest choice",
                quiz_choice,
                text_bounds(TRIAL_CHOICE_TEXT_X, 76, &"C".repeat(QUIZ_CHOICE_CHARS), 1),
            ),
            (
                "ascension heading",
                ascension_title,
                centered_text_bounds(73, "ORACLE BOND ASCENDS", 1),
            ),
            (
                "ascension tier",
                ascension_title,
                centered_text_bounds(84, "ORACLE-BOUND", 2),
            ),
            (
                "ascension level",
                ascension_level,
                centered_text_box_bounds(ASCENSION_LEVEL_BOX, "LEVEL 99", 1),
            ),
            (
                "ascension batch recap",
                ascension_batch,
                centered_text_box_bounds(ASCENSION_BATCH_BOX, "1ST TRY 99/99", 1),
            ),
            (
                "ascension controls",
                menu_footer,
                centered_text_box_bounds(MENU_FOOTER_BOX, "A / START:CONTINUE", 1),
            ),
            (
                "ascension next lenses",
                menu_footer,
                centered_text_box_bounds(MENU_FOOTER_BOX, "NEXT: INVARIANTS+TRADEOFFS", 1),
            ),
            (
                "aftermath title",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, AFTERMATH_TITLE_Y, "VISION CLOSED", 1),
            ),
            (
                "aftermath controls",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, AFTERMATH_PROMPT_Y, RESULT_PROMPT, 1),
            ),
        ] {
            assert!(
                bounds_contains(container, child),
                "{name} {child:?} exceeds its container {container:?}"
            );
            assert!(
                bounds_contains(screen, child),
                "{name} {child:?} exceeds the native frame"
            );
        }

        for (name, container, child) in [
            (
                "chronicle heading",
                chronicle_header,
                centered_compact_text_box_bounds(CHRONICLE_HEADER_BOX, "REPOSITORY CHRONICLE"),
            ),
            (
                "chronicle repository title",
                chronicle_title,
                centered_compact_text_box_bounds(CHRONICLE_TITLE_BOX, "CODE QUEST ADVANCE"),
            ),
            (
                "title first line",
                title_top,
                centered_text_box_bounds(GATEWAY_TITLE_TOP_BOX, "CODE QUEST", 2),
            ),
            (
                "title second line",
                title_bottom,
                centered_text_box_bounds(GATEWAY_TITLE_BOTTOM_BOX, "ADVANCE", 1),
            ),
            (
                "title prompt",
                title_prompt,
                centered_text_box_bounds(GATEWAY_PROMPT_BOX, "PRESS START", 1),
            ),
            (
                "title signature",
                title_signature,
                centered_text_box_bounds(GATEWAY_SIGNATURE_BOX, "REPOSITORY ORACLE", 1),
            ),
            (
                "menu heading",
                menu_heading,
                centered_text_box_bounds(GATEWAY_MENU_HEADING_BOX, "CHOOSE YOUR PATH", 1),
            ),
            (
                "menu subtitle",
                menu_subtitle,
                centered_text_box_bounds(GATEWAY_MENU_SUBTITLE_BOX, "THE BOND BEGINS HERE", 1),
            ),
            (
                "menu footer",
                menu_footer,
                centered_text_box_bounds(MENU_FOOTER_BOX, "D-PAD  A:CHOOSE  B:BACK", 1),
            ),
            (
                "menu option",
                menu_option_text,
                centered_text_box_bounds(GATEWAY_MENU_OPTION_TEXT_BOXES[1], "RETURN TO TITLE", 1),
            ),
            (
                "atelier heading",
                atelier_header,
                centered_compact_text_box_bounds(ATELIER_HEADER_BOX, "BIND YOUR CODE-SEER"),
            ),
            (
                "atelier row label",
                atelier_label,
                centered_text_box_bounds(atelier_label_box(ATELIER_ROW_BOXES[1]), "PATH", 1),
            ),
            (
                "atelier row value",
                atelier_value,
                centered_compact_text_box_bounds(
                    atelier_value_box(ATELIER_ROW_BOXES[1]),
                    "<MERGE PALADIN>",
                ),
            ),
            (
                "atelier bind action",
                atelier_bind_text,
                centered_text_box_bounds(ATELIER_BIND_TEXT_BOX, "BIND", 1),
            ),
            (
                "ascension level",
                ascension_level,
                centered_text_box_bounds(ASCENSION_LEVEL_BOX, "LEVEL 99", 1),
            ),
            (
                "ascension batch recap",
                ascension_batch,
                centered_text_box_bounds(ASCENSION_BATCH_BOX, "1ST TRY 99/99", 1),
            ),
        ] {
            assert!(
                horizontal_centers_align(container, child),
                "{name} is not horizontally centered: {child:?} in {container:?}"
            );
            assert!(
                vertical_centers_align(container, child),
                "{name} is not vertically centered: {child:?} in {container:?}"
            );
        }

        for (name, container, child) in [
            (
                "ascension heading",
                ascension_title,
                centered_text_bounds(73, "ORACLE BOND ASCENDS", 1),
            ),
            (
                "ascension tier",
                ascension_title,
                centered_text_bounds(84, "ORACLE-BOUND", 2),
            ),
            (
                "aftermath title",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, AFTERMATH_TITLE_Y, "VISION CLOSED", 1),
            ),
            (
                "aftermath controls",
                aftermath_panel,
                centered_text_in_bounds(aftermath_panel, AFTERMATH_PROMPT_Y, RESULT_PROMPT, 1),
            ),
        ] {
            assert!(
                horizontal_centers_align(container, child),
                "{name} is not horizontally centered: {child:?} in {container:?}"
            );
        }

        for (name, left, right) in [
            (
                "atelier heading and name row",
                ui_box_bounds(ATELIER_HEADER_BOX),
                ui_box_bounds(ATELIER_ROW_BOXES[0]),
            ),
            (
                "atelier label and value",
                centered_text_box_bounds(atelier_label_box(ATELIER_ROW_BOXES[1]), "PATH", 1),
                centered_compact_text_box_bounds(
                    atelier_value_box(ATELIER_ROW_BOXES[1]),
                    "<MERGE PALADIN>",
                ),
            ),
            (
                "atelier status and controls",
                text_bounds(5, 148, "VISION CLOUDY - RETRYING", 1),
                text_bounds(174, 148, "START:BIND", 1),
            ),
            (
                "trial tier and controls",
                text_bounds(44, 20, "ORACLE-BOUND", 1),
                text_bounds(126, 20, "A:ANSWER B:LEAVE", 1),
            ),
            (
                "trial batch progress and ward",
                text_bounds(44, 7, "RETRY 12/13", 1),
                trial_ward,
            ),
            (
                "trial tier and lens banner",
                text_bounds(44, 20, "ORACLE-BOUND", 1),
                text_bounds(
                    trial_banner_x("INVARIANTS RUNE III"),
                    20,
                    "INVARIANTS RUNE III",
                    1,
                ),
            ),
            ("trial ward and streak", trial_ward, trial_streak),
            (
                "trial streak and score runes",
                trial_streak,
                trial_score_runes,
            ),
            (
                "trial score runes and score",
                trial_score_runes,
                trial_score,
            ),
            (
                "sanctum tier and status",
                text_bounds(60, 5, "ORACLE-BOUND", 1),
                text_bounds(152, 5, "CLAUDE:SCRYING", 1),
            ),
            (
                "ascension heading and tier",
                centered_text_bounds(73, "ORACLE BOND ASCENDS", 1),
                centered_text_bounds(84, "ORACLE-BOUND", 2),
            ),
            (
                "trial choice ornament and copy",
                LayoutBounds {
                    x: 23,
                    y: 69,
                    width: 14,
                    height: 20,
                },
                text_bounds(TRIAL_CHOICE_TEXT_X, 76, &"C".repeat(QUIZ_CHOICE_CHARS), 1),
            ),
        ] {
            assert!(
                bounds_are_disjoint(left, right),
                "{name} overlap: {left:?} and {right:?}"
            );
        }

        // The widest batch counter clears the ward meter, and the widest lens
        // banner is pulled left inside the header outline while keeping a
        // 4px gap after the widest tier label.
        let progress = text_bounds(44, 7, "RETRY 12/13", 1);
        assert!(trial_ward.x - (progress.x + progress.width) >= 4);
        let tier = text_bounds(44, 20, "ORACLE-BOUND", 1);
        for banner in [
            "INVARIANTS RUNE III",
            "TRADEOFFS RUNE III",
            "INSIGHT III RISES",
        ] {
            let bounds = text_bounds(trial_banner_x(banner), 20, banner, 1);
            assert!(
                bounds.x - (tier.x + tier.width) >= 4,
                "{banner} crowds the tier"
            );
            assert!(
                bounds.x + bounds.width < trial_header.x + trial_header.width - 1,
                "{banner} touches the header outline"
            );
        }
        assert_eq!(trial_banner_x("A:ANSWER B:LEAVE"), TRIAL_BANNER_X);
        // The legacy counter ends before the hero token at x=47.
        assert!(text_bounds(5, 4, "R12/13", 1).width + 5 < 47);
        for (name, color, fill) in [
            ("trial retry counter", AMBER, VOID),
            ("legacy retry counter", AMBER, INK),
            ("lens banner I", mastery_rune_color(1), VOID),
            ("lens banner II", mastery_rune_color(2), VOID),
            ("lens banner III", mastery_rune_color(3), VOID),
        ] {
            let ratio = contrast_ratio(color, fill);
            assert!(ratio >= 4.5, "{name} contrast {ratio:.2}:1");
        }

        for (name, foreground) in [
            ("parchment", PARCH),
            ("mist", MIST),
            ("green", GREEN),
            ("red", RED),
            ("cyan", CYAN),
            ("secondary cyan", CYAN_DIM),
            ("amber", AMBER),
            ("magenta", MAGENTA),
        ] {
            let ratio = contrast_ratio(foreground, VOID);
            assert!(
                ratio >= 4.5,
                "{name} foreground contrast {ratio:.2}:1 is below 4.5:1"
            );
        }
    }

    #[test]
    fn oracle_templates_produce_nine_distinct_native_scene_frames() {
        let mut cartridge = oracle_template_cartridge();
        cartridge.questions[0] = QuizQuestion {
            question: "WHY SEPARATE GAME STATE FROM THE DEVICE SHELL?".into(),
            choices: vec![
                "TO KEEP RESPONSIBILITIES CLEAR".into(),
                "TO DUPLICATE RUNTIME STATE".into(),
                "TO HIDE INPUT TRANSITIONS".into(),
                "TO COUPLE RENDERING TO CSS".into(),
            ],
            answer: 0,
            ..Default::default()
        };
        let machine = SceneMachine::new((*cartridge.machine).clone());
        let mut state = GameState {
            cartridge: Some(cartridge),
            machine: Some(machine),
            quiz: Some(QuizRun {
                completed_batches: 3,
                hearts: 2,
                score: 420,
                level: 4,
                streak: 3,
                leveled_up: true,
                ledger: RunLedger {
                    first_try: 11,
                    first_try_right: 7,
                    redeemed: 2,
                    last_batch: (4, 6),
                    stages_at_start: [1, 2, 0, 1, 0],
                    ..RunLedger::default()
                },
                ..QuizRun::new()
            }),
            questions_loading: true,
            screen_ticks: 90,
            oracle_drops: vec![
                OracleDrop {
                    x: 82,
                    y: 58,
                    kind: OracleDropKind::Data,
                },
                OracleDrop {
                    x: 158,
                    y: 78,
                    kind: OracleDropKind::Bug,
                },
            ],
            ..Default::default()
        };

        let mut previews = Vec::new();
        state.screen_ticks = 60;
        let mut boot = Framebuffer::default();
        render_boot(&mut boot, &state);
        maybe_write_preview("00-boot", &boot.pixels);
        for _ in 0..180 {
            state.screen_ticks = state.screen_ticks.saturating_add(1);
            state.tick_machine();
        }
        for (name, advance_ticks) in [
            ("02a-source-ember", 90),
            ("02b-archive-answer", 54),
            ("02c-memory-vault", 66),
            ("02d-convergence", 66),
            ("02e-oracle-awakening", 78),
        ] {
            for _ in 0..advance_ticks {
                state.screen_ticks = state.screen_ticks.saturating_add(1);
                state.tick_machine();
            }
            let mut frame = Framebuffer::default();
            render_oracle_awakening(&mut frame, &state);
            maybe_write_preview(name, &frame.pixels);
            previews.push(frame.pixels);
        }
        for (name, renderer) in [
            (
                "01-chronicle",
                render_oracle_chronicle as fn(&mut Framebuffer, &GameState),
            ),
            ("03-title", render_oracle_title),
            ("04-menu", render_oracle_menu),
            ("05-atelier", render_oracle_atelier),
            ("06-sanctum", render_oracle_sanctum),
            ("07-trial", render_oracle_trial),
            ("08-ascension", render_oracle_ascension),
            ("09-aftermath", render_oracle_aftermath),
        ] {
            state.screen_ticks = match name {
                "03-title" => 60,
                _ => 90,
            };
            let mut frame = Framebuffer::default();
            renderer(&mut frame, &state);
            maybe_write_preview(name, &frame.pixels);
            previews.push(frame.pixels);
        }

        let cartridge = state.cartridge.as_mut().unwrap();
        cartridge.lessons = journal_lessons();
        cartridge.mastery = journal_mastery();
        for (name, page, revealed) in [
            ("oracle-codex-mastery", 0, false),
            ("oracle-codex-lesson-sealed", 2, false),
            ("oracle-codex-lesson", 2, true),
        ] {
            state.codex_page = page;
            state.codex_revealed = revealed;
            let mut frame = Framebuffer::default();
            render_oracle_codex(&mut frame, &state);
            maybe_write_preview(name, &frame.pixels);
            previews.push(frame.pixels);
        }
        state.codex_page = 0;
        state.codex_revealed = false;
        // With a journal, the debrief names the lens that woke this run.
        let mut ledger = Framebuffer::default();
        render_oracle_aftermath(&mut ledger, &state);
        maybe_write_preview("09b-aftermath-ledger", &ledger.pixels);

        state.quiz.as_mut().unwrap().selected = 1;
        state.quiz.as_mut().unwrap().feedback = Some((false, QUIZ_FEEDBACK_TICKS));
        state.quiz.as_mut().unwrap().retry_note = Some(RetryNote::In(RETRY_GAP));
        let mut review = Framebuffer::default();
        render_oracle_trial(&mut review, &state);
        maybe_write_preview("07b-trial-review", &review.pixels);

        state.quiz.as_mut().unwrap().feedback = None;
        state.quiz.as_mut().unwrap().retry_note = None;
        for (name, score, streak) in [
            ("07c-trial-rune-two", 900, 6),
            ("07d-trial-rune-three", 1_800, 9),
        ] {
            state.quiz.as_mut().unwrap().score = score;
            state.quiz.as_mut().unwrap().streak = streak;
            let mut threshold = Framebuffer::default();
            render_oracle_trial(&mut threshold, &state);
            maybe_write_preview(name, &threshold.pixels);
        }

        state.oracle_data = 9;
        state.oracle_bug_hits = 5;
        let mut datafall_thresholds = Framebuffer::default();
        render_oracle_sanctum(&mut datafall_thresholds, &state);
        maybe_write_preview("06b-sanctum-thresholds", &datafall_thresholds.pixels);

        let distinct = previews.iter().collect::<HashSet<_>>();
        assert_eq!(
            distinct.len(),
            16,
            "every reachable scene needs its own authored composition"
        );
        assert!(previews.iter().all(|frame| frame.len() == FRAME_BYTES));
    }
}
