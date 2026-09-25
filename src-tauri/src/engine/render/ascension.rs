use super::*;

pub(in crate::engine) const ASCENSION_TITLE_BOX: UiBox = UiBox {
    x: 47,
    y: 68,
    width: 146,
    height: 34,
};
pub(in crate::engine) const ASCENSION_LEVEL_BOX: UiBox = UiBox {
    x: 22,
    y: 117,
    width: 70,
    height: 16,
};
/// The batch recap beside the risen hero, wide enough for `1ST TRY 99/99`.
pub(in crate::engine) const ASCENSION_BATCH_BOX: UiBox = UiBox {
    x: 148,
    y: 117,
    width: 85,
    height: 16,
};

pub(in crate::engine) fn render_level_up(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Ascension) {
        render_oracle_ascension(frame, state);
        return;
    }
    frame.clear(NAVY);
    frame.outline(8, 8, 224, 144, PLUM);
    let pulse = ((state.motion_ticks() / 10) % 3) as i32;
    draw_oracle_sigil(frame, 120, 72, pulse);
    frame.centered_text(22, "LEVEL UP!", GOLD, 2);
    let rise = (state.settled_ticks(45) / 5) as i32;
    draw_hero(frame, 106, 91 - rise, 1, state);
    let level = state.quiz.as_ref().map_or(1, |run| run.level);
    frame.centered_text(42, bond_title(level), CYAN, 1);
    if let Some(run) = state.quiz.as_ref() {
        frame.centered_text(116, &format!("LEVEL {}", run.level), PARCH, 1);
        frame.centered_text(130, &first_try_label(run.ledger.last_batch), CYAN, 1);
    }
    if state.level_up_can_continue() {
        frame.centered_text(LEVEL_UP_FOOTER_Y, "A / START:CONTINUE", MIST, 1);
    } else {
        frame.centered_text(
            LEVEL_UP_FOOTER_Y,
            &next_focus_label(state.upcoming_batch_level()),
            MIST,
            1,
        );
    }
}

/// The legacy level-up footer, inside the frame like the result prompt.
pub(in crate::engine) const LEVEL_UP_FOOTER_Y: i32 = 143;

/// The level-up heading: the bond ascends only when the level crosses into a
/// new presentation tier, and deepens within one.
pub(in crate::engine) fn bond_title(level: u32) -> &'static str {
    if PresentationTier::from_level(level.saturating_sub(1)) == PresentationTier::from_level(level)
    {
        "ORACLE BOND DEEPENS"
    } else {
        "ORACLE BOND ASCENDS"
    }
}

/// The lenses the batch at `level` focuses on, e.g. `NEXT: FLOWS+TRADEOFFS`.
pub(in crate::engine) fn next_focus_label(level: u32) -> String {
    let lenses = Concept::focus_for_level(level)
        .iter()
        .map(|concept| concept.label())
        .collect::<Vec<_>>();
    format!("NEXT: {}", lenses.join("+"))
}

pub(in crate::engine) fn render_oracle_ascension(frame: &mut Framebuffer, state: &GameState) {
    let tier = state.visual_tier();
    frame.blit_rgb(ORACLE_ASCENSION);
    frame.rect(
        ASCENSION_TITLE_BOX.x,
        ASCENSION_TITLE_BOX.y,
        ASCENSION_TITLE_BOX.width,
        ASCENSION_TITLE_BOX.height,
        VOID,
    );
    frame.outline(
        ASCENSION_TITLE_BOX.x,
        ASCENSION_TITLE_BOX.y,
        ASCENSION_TITLE_BOX.width,
        ASCENSION_TITLE_BOX.height,
        AMBER,
    );
    let level = state.quiz.as_ref().map_or(1, |run| run.level);
    frame.centered_text_in(
        ASCENSION_TITLE_BOX.x,
        73,
        ASCENSION_TITLE_BOX.width,
        bond_title(level),
        AMBER,
        1,
    );
    frame.centered_text_in(
        ASCENSION_TITLE_BOX.x,
        84,
        ASCENSION_TITLE_BOX.width,
        tier.label(),
        if tier == PresentationTier::OracleBound {
            MAGENTA
        } else {
            CYAN
        },
        2,
    );
    let rise = (state.settled_ticks(45) / 5) as i32;
    draw_hero(frame, 108, 105 - rise, 1, state);
    if let Some(run) = state.quiz.as_ref() {
        frame.rect(
            ASCENSION_LEVEL_BOX.x,
            ASCENSION_LEVEL_BOX.y,
            ASCENSION_LEVEL_BOX.width,
            ASCENSION_LEVEL_BOX.height,
            VOID,
        );
        frame.outline(
            ASCENSION_LEVEL_BOX.x,
            ASCENSION_LEVEL_BOX.y,
            ASCENSION_LEVEL_BOX.width,
            ASCENSION_LEVEL_BOX.height,
            CYAN_DIM,
        );
        frame.centered_text_box(
            ASCENSION_LEVEL_BOX,
            &format!("LEVEL {}", run.level.min(99)),
            PARCH,
            1,
        );
        frame.rect(
            ASCENSION_BATCH_BOX.x,
            ASCENSION_BATCH_BOX.y,
            ASCENSION_BATCH_BOX.width,
            ASCENSION_BATCH_BOX.height,
            VOID,
        );
        frame.outline(
            ASCENSION_BATCH_BOX.x,
            ASCENSION_BATCH_BOX.y,
            ASCENSION_BATCH_BOX.width,
            ASCENSION_BATCH_BOX.height,
            CYAN_DIM,
        );
        frame.centered_text_box(
            ASCENSION_BATCH_BOX,
            &first_try_label(run.ledger.last_batch),
            PARCH,
            1,
        );
    }
    frame.rect(
        MENU_FOOTER_BOX.x,
        MENU_FOOTER_BOX.y,
        MENU_FOOTER_BOX.width,
        MENU_FOOTER_BOX.height,
        VOID,
    );
    frame.outline(
        MENU_FOOTER_BOX.x,
        MENU_FOOTER_BOX.y,
        MENU_FOOTER_BOX.width,
        MENU_FOOTER_BOX.height,
        CYAN_DIM,
    );
    if state.level_up_can_continue() {
        frame.centered_text_box(MENU_FOOTER_BOX, "A / START:CONTINUE", PARCH, 1);
    } else {
        frame.centered_text_box(
            MENU_FOOTER_BOX,
            &next_focus_label(state.upcoming_batch_level()),
            CYAN,
            1,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bond_ascends_only_across_a_tier_and_names_the_next_lenses() {
        assert_eq!(bond_title(2), "ORACLE BOND ASCENDS", "initiate to adept");
        assert_eq!(bond_title(3), "ORACLE BOND DEEPENS", "adept stays adept");
        assert_eq!(
            bond_title(4),
            "ORACLE BOND ASCENDS",
            "adept to oracle-bound"
        );
        assert_eq!(bond_title(5), "ORACLE BOND DEEPENS");
        for level in 2..=5 {
            let [first, second] = Concept::focus_for_level(level) else {
                panic!("each level focuses on two lenses");
            };
            assert_eq!(
                next_focus_label(level),
                format!("NEXT: {}+{}", first.label(), second.label())
            );
        }
        assert_eq!(next_focus_label(4), "NEXT: INVARIANTS+TRADEOFFS");
    }

    #[test]
    fn the_ascension_names_the_lenses_of_the_batch_generated_next() {
        // A saved level-3 batch was just cleared at run level 1, so the batch
        // coming next is generated at level 4, not at the run's level 2.
        let mut cartridge = oracle_template_cartridge();
        cartridge.questions = (0..QUESTION_BATCH_SIZE).map(concept_question).collect();
        let mut state = GameState {
            cartridge: Some(cartridge),
            screen: Screen::LevelUp,
            batch_ends: vec![QUESTION_BATCH_SIZE],
            batch_levels: vec![3],
            quiz: Some(QuizRun {
                level: 2,
                completed_batches: 1,
                question: QUESTION_BATCH_SIZE,
                ..QuizRun::new()
            }),
            ..Default::default()
        };
        assert_eq!(state.upcoming_batch_level(), 4);
        state.questions_loading = true;
        state.question_request_level = 4;
        assert_eq!(state.upcoming_batch_level(), 4);
        let next = next_focus_label(4);
        assert_ne!(next, next_focus_label(2));

        let mut void = Framebuffer::default();
        void.clear(VOID);
        let mut frame = Framebuffer::default();
        render_oracle_ascension(&mut frame, &state);
        assert_text_drawn(
            &frame,
            &void,
            centered_text_box_bounds(MENU_FOOTER_BOX, &next, 1),
            &next,
            CYAN,
        );
        let mut navy = Framebuffer::default();
        navy.clear(NAVY);
        let mut legacy = Framebuffer::default();
        render_level_up(&mut legacy, &state);
        assert_text_drawn(
            &legacy,
            &navy,
            centered_text_bounds(LEVEL_UP_FOOTER_Y, &next, 1),
            &next,
            MIST,
        );

        // A batch already queued names its own level.
        state.batch_ends.push(2 * QUESTION_BATCH_SIZE);
        state.batch_levels.push(5);
        assert_eq!(state.upcoming_batch_level(), 5);
    }

    #[test]
    fn level_up_requests_missing_questions_and_waits_for_the_manifest_hold() {
        use SceneHandler as H;
        use SceneSignal as S;
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        let _ = engine.take_effects();
        {
            let mut state = engine.app.world_mut().resource_mut::<GameState>();
            state.powered = true;
            state.machine = Some(SceneMachine::new(
                SceneMachineDefinition::compile(
                    "level-up",
                    vec![
                        scene_spec(
                            "level-up",
                            H::LevelUp,
                            &[(S::QuestionsReady, "quiz", Some(120))],
                        ),
                        scene_spec(
                            "quiz",
                            H::ConceptQuiz,
                            &[(S::BatchComplete, "level-up", None)],
                        ),
                    ],
                )
                .unwrap(),
            ));
            state.quiz = Some(QuizRun {
                question: 1,
                completed_batches: 1,
                level: 2,
                ..QuizRun::new()
            });
            state.transition(Screen::LevelUp);
        }

        engine.update();
        assert_eq!(request_seqs(&engine.take_effects()).len(), 1);
        deliver(&mut engine, "/tmp/engine-test", vec![concept_question(0)]);
        assert!(engine_state(&engine).has_unanswered_question());

        for _ in 0..70 {
            engine.update();
        }
        let early = {
            let mut frame = Framebuffer::default();
            render_level_up(&mut frame, engine_state(&engine));
            frame.pixels
        };
        press(&mut engine, Button::A);
        assert_eq!(
            engine.screen(),
            Screen::LevelUp,
            "A waits for the manifest's 120-tick hold"
        );
        for _ in 0..50 {
            engine.update();
        }
        let ready = {
            let mut frame = Framebuffer::default();
            render_level_up(&mut frame, engine_state(&engine));
            frame.pixels
        };
        assert_ne!(
            frame_region(&early, 0..WIDTH, 143..150),
            frame_region(&ready, 0..WIDTH, 143..150),
            "the continue prompt appears with the hold"
        );
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Quiz);
    }
}
