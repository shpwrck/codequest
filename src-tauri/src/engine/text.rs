use super::*;

pub(crate) fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if word.chars().count() > max_chars {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let chars: Vec<char> = word.chars().collect();
            for chunk in chars.chunks(max_chars) {
                lines.push(chunk.iter().collect());
            }
        } else if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub(super) fn text_width(text: &str, scale: i32) -> i32 {
    let characters = text.chars().count() as i32;
    let trailing_space = if characters > 0 { 1 } else { 0 };
    (characters * GLYPH_ADVANCE - trailing_space) * scale
}

pub(super) fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub(super) fn title_lines(title: &str) -> Vec<String> {
    let words: Vec<&str> = title
        .split(|ch: char| ch == '-' || ch == '_' || ch.is_whitespace())
        .filter(|word| !word.is_empty())
        .collect();
    let mut lines = vec![String::new()];
    for word in words {
        let candidate = if lines.last().is_some_and(|line| line.is_empty()) {
            word.to_string()
        } else {
            format!("{} {word}", lines.last().unwrap())
        };
        if candidate.chars().count() <= 19 {
            *lines.last_mut().unwrap() = candidate;
        } else if lines.len() == 1 {
            lines.push(truncate(word, 19));
        }
    }
    if lines[0].is_empty() {
        lines[0] = truncate(title, 19);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiz_copy_reserves_visible_letter_spacing_inside_its_panels() {
        assert_eq!(GLYPH_ADVANCE, GLYPH_WIDTH + 1);
        assert!(text_width(&"Q".repeat(QUIZ_QUESTION_COLUMNS), 1) <= 186);
        assert!(text_width(&"C".repeat(QUIZ_CHOICE_CHARS), 1) <= 196);
        assert_eq!(
            wrap_text(
                "WHY SEPARATE GAME STATE FROM THE DEVICE SHELL?",
                QUIZ_QUESTION_COLUMNS,
            ),
            vec!["WHY SEPARATE GAME STATE FROM", "THE DEVICE SHELL?"]
        );
    }

    #[test]
    fn wrapping_never_splits_into_oversized_lines() {
        let lines = wrap_text("alpha beta supercalifragilistic", 8);
        assert!(lines.iter().all(|line| line.chars().count() <= 8));
    }
}
