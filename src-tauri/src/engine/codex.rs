use super::*;

impl GameState {
    pub(super) fn lessons(&self) -> &[Lesson] {
        self.cartridge
            .as_ref()
            .map_or(&[], |cartridge| cartridge.lessons.as_slice())
    }

    /// Lit mastery runes (0-3) for `concept` on the inserted cartridge, gated
    /// by recent accuracy and the lens's open misses in the journal.
    pub(super) fn mastery_stage(&self, concept: Concept) -> usize {
        self.cartridge.as_ref().map_or(0, |cartridge| {
            lens_stage(&cartridge.mastery, &cartridge.lessons, concept)
        })
    }

    pub(super) fn codex_page_count(&self) -> usize {
        1 + self.lessons().len()
    }

    /// The journal lesson shown on the current Codex page, if any.
    pub(super) fn codex_lesson(&self) -> Option<(usize, &Lesson)> {
        let index = self.codex_page.checked_sub(1)?;
        self.lessons().get(index).map(|lesson| (index, lesson))
    }

    /// Whether the Codex hides `lesson`'s answer: a pending review, or a
    /// relearned lesson whose spaced check is still due, is a self-test,
    /// sealed until A reveals it on this page.
    pub(super) fn codex_answer_sealed(&self, lesson: &Lesson) -> bool {
        (lesson.outstanding || lesson.spaced_check) && !self.codex_revealed
    }

    /// Reveals the current Codex page's sealed answer. The first reveal of a
    /// pending lesson marks it peeked, so its next attempt counts as
    /// relearning, and asks the host to persist that. A page with nothing
    /// sealed is left unchanged.
    pub(super) fn reveal_codex_answer(&mut self, effects: &mut Effects) {
        let Some(index) = self
            .codex_lesson()
            .filter(|(_, lesson)| self.codex_answer_sealed(lesson))
            .map(|(index, _)| index)
        else {
            return;
        };
        self.codex_revealed = true;
        let Some(cartridge) = self.cartridge.as_mut() else {
            return;
        };
        let lesson = &mut cartridge.lessons[index];
        if !lesson.peeked {
            lesson.peeked = true;
            effects.0.push_back(EngineEffect::MarkPeeked {
                cartridge_id: cartridge.id.clone(),
                question: lesson.question.clone(),
            });
        }
    }

    /// A truthful one-line journal summary for the quiz menu, offered only when
    /// the menu can open the Codex that explains it.
    pub(super) fn journal_summary(&self) -> Option<String> {
        let lessons = self.lessons();
        if lessons.is_empty() || !self.can_signal(SceneSignal::OpenCodex) {
            return None;
        }
        let pending = lessons.iter().filter(|lesson| lesson.outstanding).count();
        Some(if pending == 0 {
            format!("LESSONS {:02}  ALL CLEAR", lessons.len().min(99))
        } else {
            format!(
                "LESSONS {:02}  REVIEW {:02}",
                lessons.len().min(99),
                pending.min(99)
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_pages_wrap_through_the_journal_without_changing_it() {
        let mut engine = quiz_menu_engine(journal_cartridge());
        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        assert_eq!(game_state(&engine).codex_page_count(), 4);
        let _ = engine.take_effects();

        for (button, expected_page) in [
            (Button::Right, 1),
            (Button::Right, 2),
            (Button::Down, 3),
            (Button::R, 0),
            (Button::Left, 3),
            (Button::Up, 2),
            (Button::L, 1),
            (Button::A, 1),
            (Button::Start, 1),
            (Button::Select, 1),
        ] {
            press(&mut engine, button);
            assert_eq!(engine.screen(), Screen::Codex, "{button:?}");
            assert_eq!(game_state(&engine).codex_page, expected_page, "{button:?}");
        }

        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        for _ in 0..30 {
            engine.update();
        }
        issue(
            &mut engine,
            EngineCommand::Input {
                button: Button::Right,
                pressed: true,
            },
        );
        assert_eq!(
            game_state(&engine).codex_page,
            2,
            "a held direction turns exactly one page"
        );

        assert!(
            engine.take_effects().is_empty(),
            "reading the Codex requests and records nothing"
        );
        let cartridge = game_state(&engine).cartridge.as_ref().unwrap();
        assert_eq!(cartridge.lessons, journal_lessons());
        assert_eq!(cartridge.mastery, journal_mastery());

        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::QuizMenu);
    }

    #[test]
    fn quiz_menu_summarizes_the_journal_when_the_codex_is_routable() {
        let mut engine = quiz_menu_engine(journal_cartridge());
        assert_eq!(
            game_state(&engine).journal_summary().as_deref(),
            Some("LESSONS 03  REVIEW 01")
        );

        let mut state = engine.app.world_mut().resource_mut::<GameState>();
        for lesson in &mut state.cartridge.as_mut().unwrap().lessons {
            lesson.outstanding = false;
        }
        assert_eq!(
            state.journal_summary().as_deref(),
            Some("LESSONS 03  ALL CLEAR")
        );
        state.cartridge.as_mut().unwrap().lessons.clear();
        assert_eq!(state.journal_summary(), None);
    }
}
