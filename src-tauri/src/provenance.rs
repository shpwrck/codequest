//! Repository provenance for the chronicle card: ranked author credits and an
//! explicit copyright notice.
//!
//! Credits must never overstate ownership. Commit authors are credited as
//! authors only; a copyright line is shown only when the repository states one
//! itself. Standard license texts carry their own template or
//! license-steward notices (the FSF line at the top of every GNU license, the
//! `[yyyy] [name of copyright owner]` appendix of Apache-2.0), and those
//! describe the license document rather than the project, so they are
//! rejected.

/// Notice files in the order they are trusted. Dedicated copyright and NOTICE
/// files state the project's own ownership; license texts come last because
/// they frequently embed template notices.
pub const NOTICE_FILES: [&str; 11] = [
    "COPYRIGHT",
    "COPYRIGHT.txt",
    "COPYRIGHT.md",
    "NOTICE",
    "NOTICE.txt",
    "NOTICE.md",
    "LICENSE",
    "LICENSE.txt",
    "LICENSE.md",
    "COPYING",
    "COPYING.txt",
];

/// Collapses whitespace, drops control characters, and bounds the length of
/// repository-supplied text before it reaches the display. A cut that lands
/// just after a space never leaves that space dangling.
pub fn sanitized_metadata(value: &str, max_chars: usize) -> String {
    let bounded = value
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect::<String>();
    bounded.trim_end().to_string()
}

/// Returns the first explicit copyright notice in a notice or license text.
pub fn copyright_notice_in(text: &str) -> Option<String> {
    let upper = text.to_ascii_uppercase();
    let gnu_license_text = [
        "GNU GENERAL PUBLIC LICENSE",
        "GNU LESSER GENERAL PUBLIC LICENSE",
        "GNU LIBRARY GENERAL PUBLIC LICENSE",
        "GNU AFFERO GENERAL PUBLIC LICENSE",
        "GNU FREE DOCUMENTATION LICENSE",
    ]
    .iter()
    .any(|title| upper.contains(title));

    text.lines().find_map(|line| {
        let notice = sanitized_metadata(line, 96);
        let holder = notice_holder(&notice)?;
        let steward_notice = gnu_license_text
            && holder
                .to_ascii_lowercase()
                .contains("free software foundation");
        (!steward_notice && !is_template_holder(holder)).then_some(notice)
    })
}

/// The holder part of a line that declares copyright, or `None` when the line
/// does not start with a copyright declaration.
fn notice_holder(notice: &str) -> Option<&str> {
    let lowercase = notice.to_ascii_lowercase();
    let rest = if let Some(rest) = lowercase.strip_prefix("spdx-filecopyrighttext:") {
        rest
    } else if let Some(rest) = lowercase.strip_prefix("copyright") {
        rest
    } else if lowercase.starts_with("(c)") || notice.starts_with('©') {
        // A bare "(c)" is also how licenses enumerate clauses ("(c) You must
        // retain..."), so it only declares copyright when a year follows.
        let rest = notice
            .trim_start_matches(['(', 'c', 'C', ')', '©'])
            .trim_start();
        if !rest.starts_with(|character: char| character.is_ascii_digit()) {
            return None;
        }
        return Some(&notice[notice.len() - rest.len()..]);
    } else {
        return None;
    };
    let mut holder = &notice[notice.len() - rest.len()..];
    loop {
        let trimmed = holder
            .trim_start_matches(|character: char| character.is_whitespace() || character == ':');
        let trimmed = ["(c)", "(C)", "©"]
            .iter()
            .find_map(|symbol| trimmed.strip_prefix(symbol))
            .unwrap_or(trimmed);
        if trimmed.len() == holder.len() {
            break;
        }
        holder = trimmed;
    }
    // "Copyright notice", "copyright holders", or a trailing "Copyright" are
    // prose about copyright, not a declaration of it.
    let first_word = holder
        .split(|character: char| !character.is_ascii_alphanumeric())
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let prose = [
        "",
        "notice",
        "notices",
        "holder",
        "holders",
        "owner",
        "owners",
        "license",
        "licenses",
        "law",
        "laws",
        "statement",
        "and",
        "or",
        "of",
        "the",
        "to",
        "in",
        "for",
        "is",
        "protection",
    ];
    (!prose.contains(&first_word.as_str())).then_some(holder)
}

/// Placeholder notices from license templates, e.g. Apache-2.0's appendix or
/// the GPL's "how to apply" section.
fn is_template_holder(holder: &str) -> bool {
    let lowercase = holder.to_ascii_lowercase();
    [
        "[yyyy]",
        "[year]",
        "<year>",
        "{yyyy}",
        "{year}",
        "yyyy",
        "name of copyright owner",
        "name of author",
        "<copyright holders>",
        "[fullname]",
        "<owner>",
    ]
    .iter()
    .any(|token| lowercase.contains(token))
}

/// Parses `git shortlog -sn` output into at most `limit` author names ranked
/// by total commits. Names are sanitized and merged case-insensitively so one
/// person committing under several spellings or addresses holds one slot.
pub fn ranked_authors(shortlog: &str, limit: usize) -> Vec<String> {
    let mut authors: Vec<(String, u64)> = Vec::new();
    for line in shortlog.lines() {
        let line = line.trim_start();
        let digits = line
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(line.len());
        let count = line[..digits].parse::<u64>().unwrap_or(0);
        let author = line[digits..].trim();
        let name = author
            .rsplit_once(" <")
            .map_or(author, |(name, _)| name)
            .trim();
        let name = sanitized_metadata(name, 64);
        if name.is_empty() {
            continue;
        }
        match authors
            .iter_mut()
            .find(|(known, _)| known.eq_ignore_ascii_case(&name))
        {
            // Saturating: counts come from text and may be absurdly large.
            Some((_, total)) => *total = total.saturating_add(count),
            None => authors.push((name, count)),
        }
    }
    // Stable: equal totals keep git's first-seen order.
    authors.sort_by_key(|(_, total)| std::cmp::Reverse(*total));
    authors
        .into_iter()
        .take(limit)
        .map(|(name, _)| name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_notices_are_credited_verbatim() {
        for (text, expected) in [
            (
                "MIT License\n\nCopyright (c) 2020-2024 Ada Lovelace\n",
                "Copyright (c) 2020-2024 Ada Lovelace",
            ),
            (
                "MIT License\n\n    Copyright (c) Microsoft Corporation.\n",
                "Copyright (c) Microsoft Corporation.",
            ),
            ("Copyright 2019 Google LLC\n", "Copyright 2019 Google LLC"),
            ("Copyright Contoso Ltd.\n", "Copyright Contoso Ltd."),
            ("© 2021 Grace Hopper\n", "© 2021 Grace Hopper"),
            ("(c) 2018 Linus\n", "(c) 2018 Linus"),
            (
                "SPDX-FileCopyrightText: 2023 The CODE QUEST Authors\n",
                "SPDX-FileCopyrightText: 2023 The CODE QUEST Authors",
            ),
        ] {
            assert_eq!(
                copyright_notice_in(text).as_deref(),
                Some(expected),
                "{text}"
            );
        }
    }

    #[test]
    fn gnu_license_texts_do_not_credit_the_license_steward() {
        let gpl = "                    GNU GENERAL PUBLIC LICENSE\n                       Version 3, 29 June 2007\n\n Copyright (C) 2007 Free Software Foundation, Inc. <https://fsf.org/>\n Everyone is permitted to copy and distribute verbatim copies\n";
        assert_eq!(copyright_notice_in(gpl), None);

        let gpl_with_project_notice =
            format!("{gpl}\n    Copyright (C) <year>  <name of author>\n\nCopyright (C) 2022 Real Project\n");
        assert_eq!(
            copyright_notice_in(&gpl_with_project_notice).as_deref(),
            Some("Copyright (C) 2022 Real Project")
        );

        // The Free Software Foundation remains creditable for its own projects.
        assert_eq!(
            copyright_notice_in("Copyright (C) 1989 Free Software Foundation, Inc.\n").as_deref(),
            Some("Copyright (C) 1989 Free Software Foundation, Inc.")
        );
    }

    #[test]
    fn license_templates_and_prose_are_not_notices() {
        let apache = "   4. Redistribution.\n\n      (c) You must retain, in the Source form of any Derivative Works\n\n   APPENDIX: How to apply the Apache License to your work.\n\n   Copyright [yyyy] [name of copyright owner]\n";
        assert_eq!(copyright_notice_in(apache), None);

        for prose in [
            "The above copyright notice and this permission notice shall be included.",
            "Copyright notice",
            "COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\"",
            "Copyright and related rights waived via CC0.",
            "Copyright",
            "Copyright (C) <year> <name of author>",
            "Copyright (c) {yyyy} {name of copyright owner}",
        ] {
            assert_eq!(copyright_notice_in(prose), None, "{prose}");
        }
    }

    #[test]
    fn author_credits_merge_one_person_across_addresses_and_rank_by_total() {
        let shortlog = "    12\tGrace Hopper\n     9\tAda Lovelace\n     8\tada lovelace\n     1\t\n     1\tKatherine Johnson\n";
        assert_eq!(
            ranked_authors(shortlog, 3),
            ["Ada Lovelace", "Grace Hopper", "Katherine Johnson"]
        );
        assert_eq!(ranked_authors(shortlog, 1), ["Ada Lovelace"]);
    }

    #[test]
    fn author_credits_accept_email_shortlog_and_strip_addresses() {
        let shortlog = "     3\tAda Lovelace <ada@example.com>\n     2\tAda Lovelace <ada@users.noreply.github.com>\n     4\tGrace Hopper <grace@example.com>\n";
        assert_eq!(
            ranked_authors(shortlog, 3),
            ["Ada Lovelace", "Grace Hopper"]
        );
    }

    #[test]
    fn metadata_is_sanitized_and_bounded() {
        assert_eq!(
            sanitized_metadata("  main\u{7}\n branch  ", 48),
            "main branch"
        );
        assert_eq!(sanitized_metadata("abcdefgh", 3), "abc");
    }

    #[test]
    fn a_length_cut_after_a_space_leaves_no_trailing_space() {
        assert_eq!(sanitized_metadata("Ada Lovelace", 4), "Ada");
        let long_name = format!("{} Lovelace", "A".repeat(63));
        assert_eq!(
            ranked_authors(&format!("     1\t{long_name}\n"), 1),
            ["A".repeat(63)]
        );
    }

    #[test]
    fn huge_commit_counts_saturate_instead_of_overflowing() {
        let shortlog = "18446744073709551615\tAda Lovelace\n18446744073709551615\tada lovelace\n     7\tGrace Hopper\n";
        assert_eq!(
            ranked_authors(shortlog, 2),
            ["Ada Lovelace", "Grace Hopper"]
        );
    }

    #[test]
    fn the_first_of_several_project_notices_is_credited() {
        assert_eq!(
            copyright_notice_in("Copyright 2020 Original Author\nCopyright 2023 Later Fork\n")
                .as_deref(),
            Some("Copyright 2020 Original Author")
        );
    }

    #[test]
    fn every_prose_opening_after_copyright_is_not_a_notice() {
        for word in [
            "notice",
            "notices",
            "holder",
            "holders",
            "owner",
            "owners",
            "license",
            "licenses",
            "law",
            "laws",
            "statement",
            "and",
            "or",
            "of",
            "the",
            "to",
            "in",
            "for",
            "is",
            "protection",
        ] {
            let prose = format!("Copyright {word} applies to this work.");
            assert_eq!(copyright_notice_in(&prose), None, "{prose}");
        }
    }

    #[test]
    fn every_template_placeholder_is_not_a_notice() {
        for placeholder in [
            "[yyyy]",
            "[year]",
            "<year>",
            "{yyyy}",
            "{year}",
            "yyyy",
            "name of copyright owner",
            "name of author",
            "<copyright holders>",
            "[fullname]",
            "<owner>",
        ] {
            // A leading year makes the holder look real to the prose check,
            // so only the placeholder check can reject these.
            let template = format!("Copyright (c) 2024 {placeholder}");
            assert_eq!(copyright_notice_in(&template), None, "{template}");
        }
    }
}
