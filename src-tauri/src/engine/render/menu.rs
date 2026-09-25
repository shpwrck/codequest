use super::*;

pub(in crate::engine) const GATEWAY_MENU_HEADING_BOX: UiBox = UiBox {
    x: 54,
    y: 49,
    width: 132,
    height: 7,
};
pub(in crate::engine) const GATEWAY_MENU_SUBTITLE_BOX: UiBox = UiBox {
    x: 54,
    y: 62,
    width: 132,
    height: 7,
};
pub(in crate::engine) const GATEWAY_MENU_OPTION_BOXES: [UiBox; 2] = [
    UiBox {
        x: 59,
        y: 92,
        width: 121,
        height: 20,
    },
    UiBox {
        x: 59,
        y: 119,
        width: 121,
        height: 20,
    },
];
pub(in crate::engine) const GATEWAY_MENU_OPTION_TEXT_BOXES: [UiBox; 2] = [
    UiBox {
        x: 66,
        y: 94,
        width: 108,
        height: 14,
    },
    UiBox {
        x: 66,
        y: 121,
        width: 108,
        height: 14,
    },
];

pub(in crate::engine) fn quiz_menu_second_option(state: &GameState) -> &'static str {
    if state.can_signal(SceneSignal::OpenCodex) {
        "OPEN THE CODEX"
    } else {
        "RETURN TO TITLE"
    }
}

pub(in crate::engine) fn render_quiz_menu(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Menu) {
        render_oracle_menu(frame, state);
        return;
    }
    frame.clear(NAVY);
    frame.centered_text(20, "REPO QUIZ", GOLD, 2);
    if let Some(summary) = state.journal_summary() {
        frame.centered_text(44, &summary, SKY, 1);
    }
    frame.outline(34, 58, 172, 62, SKY);
    for (index, label) in ["BEGIN RUN", quiz_menu_second_option(state)]
        .iter()
        .enumerate()
    {
        let y = 74 + index as i32 * 24;
        if state.menu_selected == index {
            frame.rect(43, y - 3, 154, 14, ROYAL);
            frame.text(47, y, ">", GOLD, 1);
        }
        frame.text(59, y, label, PARCH, 1);
    }
    frame.centered_text(140, "A:CHOOSE  B:BACK", MIST, 1);
}

pub(in crate::engine) fn render_oracle_menu(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_GATEWAY);
    frame.centered_text_box(GATEWAY_MENU_HEADING_BOX, "CHOOSE YOUR PATH", PARCH, 1);
    let subtitle = state
        .journal_summary()
        .unwrap_or_else(|| "THE BOND BEGINS HERE".into());
    frame.centered_text_box(GATEWAY_MENU_SUBTITLE_BOX, &subtitle, CYAN, 1);
    for (index, label) in ["BEGIN THE TRIAL", quiz_menu_second_option(state)]
        .iter()
        .enumerate()
    {
        let option_box = GATEWAY_MENU_OPTION_BOXES[index];
        let text_box = GATEWAY_MENU_OPTION_TEXT_BOXES[index];
        let focused = state.menu_selected == index;
        if focused {
            draw_asset_focus(
                frame,
                option_box.x,
                option_box.y,
                option_box.width,
                option_box.height,
            );
        }
        frame.centered_text_box(text_box, label, if focused { PARCH } else { MIST }, 1);
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
    frame.centered_text_box(MENU_FOOTER_BOX, "D-PAD  A:CHOOSE  B:BACK", MIST, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiz_menu_opens_the_codex_when_the_scene_graph_routes_to_it() {
        let mut engine = quiz_menu_engine(journal_cartridge());
        assert_eq!(
            quiz_menu_second_option(game_state(&engine)),
            "OPEN THE CODEX"
        );

        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Codex);
        assert_eq!(game_state(&engine).codex_page, 0);

        press(&mut engine, Button::Right);
        press(&mut engine, Button::B);
        assert_eq!(engine.screen(), Screen::QuizMenu);
        assert_eq!(
            game_state(&engine).menu_selected,
            1,
            "returning from the Codex keeps its menu option focused"
        );

        press(&mut engine, Button::Start);
        assert_eq!(engine.screen(), Screen::Codex);
        assert_eq!(
            game_state(&engine).codex_page,
            0,
            "every visit opens on the mastery overview"
        );

        press(&mut engine, Button::B);
        press(&mut engine, Button::B);
        assert_eq!(
            engine.screen(),
            Screen::Title,
            "B on the menu still returns to the title"
        );
    }

    #[test]
    fn quiz_menu_without_a_codex_route_keeps_return_to_title() {
        let route = |signal, target: &str| SceneTransition {
            signal,
            target: target.into(),
            after_ticks: None,
        };
        let machine = SceneMachineDefinition::compile(
            "title",
            vec![
                SceneSpec {
                    id: "title".into(),
                    handler: SceneHandler::Title,
                    transitions: vec![route(SceneSignal::Continue, "quiz-menu")],
                },
                SceneSpec {
                    id: "quiz-menu".into(),
                    handler: SceneHandler::QuizMenu,
                    transitions: vec![
                        route(SceneSignal::NewRun, "character-creation"),
                        route(SceneSignal::Back, "title"),
                    ],
                },
                SceneSpec {
                    id: "character-creation".into(),
                    handler: SceneHandler::CharacterCreation,
                    transitions: vec![route(SceneSignal::Back, "quiz-menu")],
                },
            ],
        )
        .unwrap();
        let mut cartridge = journal_cartridge();
        cartridge.machine = Box::new(machine);
        let mut engine = GameEngine::new();
        issue(&mut engine, EngineCommand::Cartridge(Some(cartridge)));
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        press(&mut engine, Button::Start);
        assert_eq!(engine.screen(), Screen::QuizMenu);

        let state = game_state(&engine);
        assert_eq!(quiz_menu_second_option(state), "RETURN TO TITLE");
        assert_eq!(
            state.journal_summary(),
            None,
            "a menu without the Codex keeps its original subtitle"
        );
        press(&mut engine, Button::Down);
        press(&mut engine, Button::A);
        assert_eq!(engine.screen(), Screen::Title);
    }
}
