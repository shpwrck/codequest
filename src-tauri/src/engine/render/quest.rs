use super::*;

pub(in crate::engine) fn render_quest_select(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(NAVY);
    frame.rect(0, 0, WIDTH as i32, 22, INK);
    frame.centered_text(7, "CHOOSE THY QUEST", GOLD, 1);
    let Some(cart) = state.cartridge.as_ref() else {
        return;
    };
    if cart.quests.is_empty() {
        frame.centered_text(70, "NO QUESTS ON CARTRIDGE", RED, 1);
        return;
    }
    let start = state.quest_selected.saturating_sub(2);
    for (row, quest) in cart.quests.iter().skip(start).take(5).enumerate() {
        let index = start + row;
        let y = 31 + row as i32 * 22;
        if index == state.quest_selected {
            frame.rect(4, y - 3, 232, 18, ROYAL);
            frame.text(8, y, ">", GOLD, 1);
        }
        frame.text(20, y, &truncate(&quest.name, 35), PARCH, 1);
        frame.text(20, y + LINE_HEIGHT, &truncate(&quest.boss, 35), MIST, 1);
    }
    frame.centered_text(150, "A:FIGHT  B:BACK", MIST, 1);
}

pub(in crate::engine) fn render_battle(frame: &mut Framebuffer, state: &GameState) {
    frame.clear(NAVY);
    frame.rect(0, 0, WIDTH as i32, 69, INK);
    frame.text(6, 5, &truncate(&state.active_boss, 37), RED, 1);
    draw_crab(frame, 34, 45, 1);
    draw_boss(frame, 183, 29, state.motion_ticks(), 1);
    frame.rect(0, 68, WIDTH as i32, 2, SKY);
    frame.outline(4, 75, 232, 68, MIST);
    for (index, (line, stderr)) in state.logs.iter().rev().take(7).rev().enumerate() {
        frame.text(
            9,
            80 + index as i32 * LINE_HEIGHT,
            &truncate(line, 37),
            if *stderr { RED } else { PARCH },
            1,
        );
    }
    frame.text(5, 151, "B:ABORT", GOLD, 1);
    frame.text(164, 151, "RUST PROCESS", GREEN, 1);
}

pub(in crate::engine) fn render_result(frame: &mut Framebuffer, success: bool) {
    frame.clear(if success { NAVY } else { INK });
    frame.centered_text(
        39,
        if success { "QUEST" } else { "GAME" },
        if success { GOLD } else { RED },
        2,
    );
    frame.centered_text(
        60,
        if success { "CLEARED" } else { "OVER" },
        if success { GREEN } else { RED },
        2,
    );
    frame.outline(31, 91, 178, 33, if success { SKY } else { PLUM });
    frame.centered_text(103, "A:QUEST LIST", PARCH, 1);
}
