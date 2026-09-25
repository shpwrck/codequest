use super::*;

pub(in crate::engine) const CHRONICLE_HEADER_BOX: UiBox = UiBox {
    x: 68,
    y: 39,
    width: 104,
    height: 7,
};
pub(in crate::engine) const CHRONICLE_TITLE_BOX: UiBox = UiBox {
    x: 62,
    y: 49,
    width: 116,
    height: 16,
};
pub(in crate::engine) const CHRONICLE_COPYRIGHT_BOX: UiBox = UiBox {
    x: 62,
    y: 67,
    width: 116,
    height: 16,
};
pub(in crate::engine) const CHRONICLE_AUTHORS_LABEL_BOX: UiBox = UiBox {
    x: 62,
    y: 85,
    width: 116,
    height: 7,
};
pub(in crate::engine) const CHRONICLE_AUTHORS_BOX: UiBox = UiBox {
    x: 62,
    y: 94,
    width: 116,
    height: 23,
};
pub(in crate::engine) const GATEWAY_TITLE_TOP_BOX: UiBox = UiBox {
    x: 56,
    y: 49,
    width: 128,
    height: 14,
};
pub(in crate::engine) const GATEWAY_TITLE_BOTTOM_BOX: UiBox = UiBox {
    x: 63,
    y: 67,
    width: 114,
    height: 7,
};
pub(in crate::engine) const GATEWAY_PROMPT_BOX: UiBox = UiBox {
    x: 68,
    y: 98,
    width: 104,
    height: 7,
};
pub(in crate::engine) const GATEWAY_SIGNATURE_BOX: UiBox = UiBox {
    x: 68,
    y: 123,
    width: 104,
    height: 7,
};

pub(in crate::engine) fn render_boot(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb_graded(ORACLE_GATEWAY, 92, 105, 74);
    frame.centered_text_box(GATEWAY_TITLE_TOP_BOX, "CODE QUEST", PARCH, 2);
    frame.centered_text_box(GATEWAY_TITLE_BOTTOM_BOX, "ADVANCE", AMBER, 1);
    frame.centered_text_box(GATEWAY_SIGNATURE_BOX, "REPOSITORY ORACLE", CYAN_DIM, 1);
    if !state.has_game() && state.screen_ticks > 50 && state.blink_lit(30) {
        frame.centered_text_box(GATEWAY_PROMPT_BOX, "INSERT CARTRIDGE", PARCH, 1);
    }
}

pub(in crate::engine) fn render_oracle_chronicle(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_CHRONICLE);
    frame.centered_compact_text_box(CHRONICLE_HEADER_BOX, "REPOSITORY CHRONICLE", MIST);
    if state.screen_ticks >= 42 {
        frame.rect(42, 122, 156, 17, VOID);
        frame.outline(42, 122, 156, 17, CYAN_DIM);
    }

    let title = state
        .cartridge
        .as_ref()
        .map_or("NO CARTRIDGE", |cartridge| cartridge.title.as_str());
    let title_lines = wrap_text(title, 23)
        .into_iter()
        .map(|line| truncate(&line, 23))
        .collect::<Vec<_>>();
    frame.centered_compact_lines_box(CHRONICLE_TITLE_BOX, &title_lines, PARCH);

    if let Some(provenance) = state
        .cartridge
        .as_ref()
        .map(|cartridge| &cartridge.provenance)
    {
        if state.screen_ticks >= 12 {
            let mut notice = provenance
                .copyright
                .as_deref()
                .map(|notice| notice.replace('©', "(C)"))
                .unwrap_or_else(|| "NO DECLARED COPYRIGHT NOTICE".into());
            if notice
                .get(..10)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("copyright "))
            {
                notice.drain(..10);
            }
            notice = notice.replace("(C)", "COPY").replace("(c)", "COPY");
            let notice_lines = wrap_text(&notice, 23)
                .into_iter()
                .map(|line| truncate(&line, 23))
                .collect::<Vec<_>>();
            frame.centered_compact_lines_box(CHRONICLE_COPYRIGHT_BOX, &notice_lines, MIST);
        }
        if state.screen_ticks >= 24 {
            frame.centered_compact_text_box(
                CHRONICLE_AUTHORS_LABEL_BOX,
                "COMMIT AUTHORS",
                CYAN_DIM,
            );
            if provenance.authors.is_empty() {
                frame.centered_compact_text_box(CHRONICLE_AUTHORS_BOX, "NO AUTHORS YET", PARCH);
            } else {
                let authors = provenance
                    .authors
                    .iter()
                    .take(3)
                    .map(|author| truncate(author, 23))
                    .collect::<Vec<_>>();
                frame.centered_compact_lines_box(CHRONICLE_AUTHORS_BOX, &authors, PARCH);
            }
        }
        if state.screen_ticks >= 42 {
            let history = match (provenance.first_year, provenance.latest_year) {
                (Some(first), Some(latest)) if first == latest => format!("ARCHIVE YEAR {first}"),
                (Some(first), Some(latest)) => format!("ARCHIVE {first} > {latest}"),
                _ => "HISTORY NOT YET WRITTEN".into(),
            };
            frame.centered_text_in(42, 126, 156, &truncate(&history, 24), MIST, 1);
        }
    }
    if state.can_signal(SceneSignal::Continue) && state.blink_lit(20) {
        frame.rect(74, 141, 92, 16, VOID);
        frame.outline(74, 141, 92, 16, CYAN_DIM);
        frame.centered_text_in(74, 145, 92, "A / START:SKIP", MIST, 1);
    }
}

pub(in crate::engine) fn render_oracle_opening_beat(
    frame: &mut Framebuffer,
    beat: OpeningBeat,
    ticks: u64,
) {
    match beat {
        OpeningBeat::Legacy => frame.blit_awakening(ticks),
        OpeningBeat::SourceEmber => frame.blit_rgb(ORACLE_AWAKENING_SOURCE),
        OpeningBeat::ArchiveAnswer => frame.blit_rgb(ORACLE_AWAKENING_SIGNAL),
        OpeningBeat::MemoryVault => frame.blit_rgb(ORACLE_AWAKENING_ARCHIVE),
        OpeningBeat::Convergence => frame.blit_rgb(ORACLE_AWAKENING_CONVERGENCE),
        OpeningBeat::OracleAwakening => frame.blit_awakening(ticks.saturating_add(188)),
    }
}

pub(in crate::engine) fn render_oracle_awakening(frame: &mut Framebuffer, state: &GameState) {
    render_oracle_opening_beat(frame, state.opening_beat(), state.screen_ticks);
    if state.can_signal(SceneSignal::Continue) {
        frame.rect(164, 149, 76, 11, VOID);
        frame.text(166, 151, "A/START:SKIP", MIST, 1);
    }
}

pub(in crate::engine) fn render_oracle_title(frame: &mut Framebuffer, state: &GameState) {
    frame.blit_rgb(ORACLE_GATEWAY);
    let title = state
        .cartridge
        .as_ref()
        .map_or("NO CARTRIDGE", |cart| cart.title.as_str());
    let lines = wrap_text(title, 10);
    frame.centered_text_box(GATEWAY_TITLE_TOP_BOX, &truncate(&lines[0], 10), PARCH, 2);
    if let Some(line) = lines.get(1) {
        frame.centered_text_box(GATEWAY_TITLE_BOTTOM_BOX, &truncate(line, 19), AMBER, 1);
    }
    frame.centered_text_box(GATEWAY_SIGNATURE_BOX, "REPOSITORY ORACLE", CYAN, 1);
    if state.has_game() && state.blink_lit(30) {
        frame.centered_text_box(GATEWAY_PROMPT_BOX, "PRESS START", PARCH, 1);
    }
}

pub(in crate::engine) fn render_copyright(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Chronicle) {
        render_oracle_chronicle(frame, state);
        return;
    }
    frame.clear(INK);
    frame.outline(8, 8, 224, 144, GOLD);
    frame.outline(12, 12, 216, 136, NAVY);
    frame.centered_text(20, "REPOSITORY CHRONICLE", GOLD, 1);

    let title = state
        .cartridge
        .as_ref()
        .map_or("NO CARTRIDGE", |cartridge| cartridge.title.as_str());
    let lines = title_lines(title);
    frame.centered_text(40, &lines[0], PARCH, 1);
    if let Some(line) = lines.get(1) {
        frame.centered_text(51, line, PARCH, 1);
    }

    if let Some(provenance) = state
        .cartridge
        .as_ref()
        .map(|cartridge| &cartridge.provenance)
    {
        let notice = provenance
            .copyright
            .as_deref()
            .map(|notice| notice.replace('©', "(C)"))
            .unwrap_or_else(|| "NO COPYRIGHT NOTICE FOUND".into());
        frame.centered_text(66, &truncate(&notice, 35), MIST, 1);
        frame.rect(42, 79, 156, 1, PLUM);
        frame.centered_text(86, "AUTHORS", SKY, 1);

        if provenance.authors.is_empty() {
            frame.centered_text(99, "NO COMMIT AUTHORS YET", PARCH, 1);
        } else {
            for (index, author) in provenance.authors.iter().take(3).enumerate() {
                frame.centered_text(98 + index as i32 * 10, &truncate(author, 32), PARCH, 1);
            }
        }

        let history = match (provenance.first_year, provenance.latest_year) {
            (Some(first), Some(latest)) if first == latest => format!("HISTORY {first}"),
            (Some(first), Some(latest)) => format!("HISTORY {first}-{latest}"),
            _ => "HISTORY NOT YET WRITTEN".into(),
        };
        frame.centered_text(129, &history, GOLD, 1);
    }
    if state.can_signal(SceneSignal::Continue) && state.blink_lit(20) {
        frame.centered_text(141, "START:SKIP", PARCH, 1);
    }
}

pub(in crate::engine) fn render_opening_fanfare(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Awakening) {
        render_oracle_awakening(frame, state);
        return;
    }
    // Story beats keep their timing; only decorative motion honors reduced motion.
    let ticks = state.screen_ticks;
    let motion = state.motion_ticks();
    frame.clear(INK);
    for index in 0..24 {
        let x = ((index * 67 + motion as usize) % WIDTH) as i32;
        let y = ((index * 43 + 17) % HEIGHT) as i32;
        frame.pixel(x, y, if index % 4 == 0 { GOLD } else { MIST });
    }

    if ticks < 120 {
        let travel = (state.settled_ticks(110) as i32 * 70) / 110;
        draw_code_sigil(frame, 24 + travel, 78, false);
        draw_code_sigil(frame, 216 - travel, 78, true);
        if ticks >= 100 {
            let flare = if state.reduced_motion {
                8
            } else {
                ((ticks - 100) as i32 / 4).min(8)
            };
            frame.rect(120 - flare, 78 - 1, flare * 2 + 1, 3, PARCH);
            frame.rect(119, 79 - flare, 3, flare * 2 + 1, GOLD);
        }
        frame.centered_text(132, "TWO PATHS CONVERGE", SKY, 1);
    } else {
        draw_commit_constellation(frame, motion);
        draw_oracle_sigil(frame, 120, 78, ((motion / 10) % 3) as i32);
        frame.centered_text(20, "HISTORY BECOMES POWER", GOLD, 1);
        frame.centered_text(135, "THE ORACLE OPENS", PARCH, 1);
    }

    if state.can_signal(SceneSignal::Continue) {
        frame.text(176, 149, "START:SKIP", MIST, 1);
    }
}

pub(in crate::engine) fn render_title(frame: &mut Framebuffer, state: &GameState) {
    if state.uses_visual_template(VisualTemplate::Title) {
        render_oracle_title(frame, state);
        return;
    }
    frame.clear(NAVY);
    for index in 0..42 {
        let x = ((index * 53 + state.motion_ticks() as usize / 3) % WIDTH) as i32;
        let y = ((index * 37 + 11) % HEIGHT) as i32;
        frame.pixel(x, y, if index % 3 == 0 { SKY } else { MIST });
    }
    let title = state
        .cartridge
        .as_ref()
        .map_or("NO CARTRIDGE", |cart| cart.title.as_str());
    let lines = title_lines(title);
    frame.centered_text(42, &lines[0], PARCH, 2);
    if let Some(line) = lines.get(1) {
        frame.centered_text(61, line, GOLD, 2);
    }
    let subtitle = match state.cartridge_mode() {
        Some(CartridgeMode::Quiz) => "ENDLESS REPO QUIZ",
        Some(CartridgeMode::Custom) => "EVERY COMMAND IS A BOSS",
        None => "POWER OFF TO LOAD A GAME",
    };
    frame.centered_text(91, subtitle, SKY, 1);
    if state.has_game() && state.blink_lit(30) {
        frame.centered_text(126, "PRESS START", PARCH, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_scenes_render_distinct_frames() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        let boot = engine.frame().to_vec();

        issue(&mut engine, EngineCommand::BootComplete);
        let copyright = engine.frame().to_vec();
        assert_ne!(copyright, boot);

        for _ in 0..179 {
            engine.update();
        }
        let fanfare_impact = engine.frame().to_vec();
        assert_ne!(fanfare_impact, copyright);

        for _ in 0..120 {
            engine.update();
        }
        let fanfare_oracle = engine.frame().to_vec();
        assert_ne!(fanfare_oracle, fanfare_impact);

        for _ in 0..210 {
            engine.update();
        }
        assert_eq!(engine.screen(), Screen::Title);
        assert_ne!(engine.frame(), fanfare_oracle);
    }

    #[test]
    fn oracle_opening_earns_its_brightest_frame_from_a_dormant_start() {
        let mut state = GameState::default();
        let cartridge = oracle_template_cartridge();
        state.machine = Some(SceneMachine::new((*cartridge.machine).clone()));
        state.cartridge = Some(cartridge);
        state.transition(Screen::Copyright);
        let mut frame = Framebuffer::default();

        render_copyright(&mut frame, &state);
        let dormant_bright = color_pixels_in_region(&frame.pixels, CYAN, 0..WIDTH, 0..HEIGHT)
            + color_pixels_in_region(&frame.pixels, AMBER, 0..WIDTH, 0..HEIGHT);
        assert_eq!(
            dormant_bright, 0,
            "the archive begins without emissive light"
        );

        let beats = [
            OpeningBeat::SourceEmber,
            OpeningBeat::ArchiveAnswer,
            OpeningBeat::MemoryVault,
            OpeningBeat::Convergence,
            OpeningBeat::OracleAwakening,
        ];
        let mut frames = Vec::new();
        let mut luminance = Vec::new();
        for beat in beats {
            render_oracle_opening_beat(&mut frame, beat, 66);
            luminance.push(total_luminance(&frame.pixels));
            frames.push(frame.pixels.clone());
        }

        assert!(
            luminance.windows(2).all(|pair| pair[0] < pair[1]),
            "each authored story scene must increase luminance toward the Oracle climax: {luminance:?}"
        );
        assert_eq!(
            frames.iter().collect::<HashSet<_>>().len(),
            5,
            "the opening must render five distinct authored scenes"
        );
    }

    #[test]
    fn late_fanfare_keeps_its_own_frame_until_title_transition() {
        let mut engine = GameEngine::new();
        issue(
            &mut engine,
            EngineCommand::Cartridge(Some(quiz_cartridge())),
        );
        issue(&mut engine, EngineCommand::Power(true));
        issue(&mut engine, EngineCommand::BootComplete);
        for _ in 0..179 {
            engine.update();
        }
        for _ in 0..240 {
            engine.update();
        }

        assert_eq!(engine.screen(), Screen::OpeningFanfare);
        assert_eq!(&engine.frame()[..4], &[INK.0, INK.1, INK.2, 255]);
    }

    #[test]
    fn copyright_frame_reflects_cartridge_provenance() {
        let mut first_cartridge = quiz_cartridge();
        first_cartridge.provenance.authors = vec!["ADA LOVELACE".into()];
        first_cartridge.provenance.first_year = Some(1842);
        first_cartridge.provenance.latest_year = Some(1843);
        let mut first = GameEngine::new();
        issue(&mut first, EngineCommand::Cartridge(Some(first_cartridge)));
        issue(&mut first, EngineCommand::Power(true));
        issue(&mut first, EngineCommand::BootComplete);

        let mut second_cartridge = quiz_cartridge();
        second_cartridge.provenance.authors = vec!["GRACE HOPPER".into()];
        second_cartridge.provenance.first_year = Some(1944);
        second_cartridge.provenance.latest_year = Some(1992);
        let mut second = GameEngine::new();
        issue(
            &mut second,
            EngineCommand::Cartridge(Some(second_cartridge)),
        );
        issue(&mut second, EngineCommand::Power(true));
        issue(&mut second, EngineCommand::BootComplete);

        assert_ne!(first.frame(), second.frame());
    }
}
