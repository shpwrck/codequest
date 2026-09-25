//! A deterministic, budgeted project brief for question generation.
//!
//! The brief tells a question provider what a project is for and how it is
//! shaped without revealing where anything lives. Documentation is excerpted
//! as prose, design notes are reduced to headings and leading paragraphs, and
//! source files are reduced to their doc comments and top-level signatures.
//! Every excerpt is labelled by number, never by path or file name, and
//! path-like words inside excerpts are redacted. The same repository contents
//! always produce the same brief, and the brief never exceeds
//! [`BRIEF_BUDGET`] bytes of plain ASCII.

use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path};

use crate::{codequest, questions};

/// Bytes available to the whole brief. The brief travels inside a single
/// command-line prompt (see `questions::MAX_PROMPT_CHARS`), which bounds it.
pub(crate) const BRIEF_BUDGET: usize = 20 * 1024;
const SUMMARY_BUDGET: usize = 1024;
const README_BUDGET: usize = 8 * 1024;
const DESIGN_NOTES_BUDGET: usize = 5 * 1024;
const DESIGN_NOTE_BUDGET: usize = 1536;
const MAX_COMPONENTS: usize = 30;
/// The most one component may take; smaller skeletons leave their unused
/// share to larger ones.
const COMPONENT_BUDGET: usize = 1600;
const MIN_COMPONENT_BUDGET: usize = 160;
/// Design notes stop once less than this remains of their share.
const MIN_NOTE_BUDGET: usize = 160;
/// Source files read while looking for components with a usable skeleton.
const MAX_COMPONENT_READS: usize = 4 * MAX_COMPONENTS;
/// Files larger than this are generated, vendored, or data, not design.
const MAX_TEXT_BYTES: u64 = 512 * 1024;
const PARAGRAPH_CHARS: usize = 320;
const ABOUT_CHARS: usize = 300;
const DOC_SENTENCE_CHARS: usize = 100;
const SIGNATURE_CHARS: usize = 110;
/// Continuation lines joined into one multi-line signature.
const MAX_SIGNATURE_LINES: usize = 8;

const MARKDOWN_EXTENSIONS: [&str; 5] = ["md", "markdown", "mdx", "rst", "adoc"];
/// Directory names whose contents are third-party, generated, or build output.
const EXCLUDED_DIRECTORIES: [&str; 22] = [
    "node_modules",
    "vendor",
    "vendors",
    "third_party",
    "third-party",
    "thirdparty",
    "external",
    "extern",
    "deps",
    "target",
    "dist",
    "build",
    "out",
    "obj",
    "gen",
    "generated",
    "__generated__",
    "__pycache__",
    "venv",
    "site-packages",
    "bower_components",
    "coverage",
];
/// Directories of supporting code (tooling, packaging, samples). Their files
/// are picked only after the project's own components.
const AUXILIARY_DIRECTORIES: [&str; 22] = [
    "scripts",
    "script",
    "tools",
    "tool",
    "packaging",
    "ci",
    "docs",
    "doc",
    "examples",
    "example",
    "samples",
    "sample",
    "demo",
    "demos",
    "benchmarks",
    "bench",
    "migrations",
    "deploy",
    "deployment",
    "infra",
    "hack",
    "contrib",
];
const TEST_DIRECTORIES: [&str; 9] = [
    "test",
    "tests",
    "__tests__",
    "spec",
    "specs",
    "testdata",
    "fixtures",
    "e2e",
    "benches",
];
/// Documents about releases, people, or policy rather than design.
const NON_DESIGN_DOCUMENTS: [&str; 20] = [
    "CHANGELOG",
    "CHANGES",
    "HISTORY",
    "LICENSE",
    "LICENCE",
    "COPYING",
    "CODE_OF_CONDUCT",
    "SECURITY",
    "NOTICE",
    "AUTHORS",
    "CONTRIBUTORS",
    "MAINTAINERS",
    "RELEASE",
    "RELEASES",
    "RELEASE_NOTES",
    "PULL_REQUEST_TEMPLATE",
    "ISSUE_TEMPLATE",
    "CODEOWNERS",
    "FUNDING",
    "SUPPORT",
];
const DESIGN_DOCUMENTS: [&str; 5] = ["ARCHITECTURE", "DESIGN", "OVERVIEW", "CONCEPTS", "GLOSSARY"];
const DECISION_DIRECTORIES: [&str; 4] = ["adr", "adrs", "decisions", "decision-records"];
const PROCEDURE_DIRECTORIES: [&str; 5] = ["runbook", "runbooks", "howto", "how-to", "playbooks"];
/// Markdown sections about obtaining, building, or licensing a project. They
/// describe operations rather than design, so excerpts skip them.
const OPERATIONAL_SECTIONS: [&str; 20] = [
    "INSTALL",
    "INSTALLATION",
    "INSTALLING",
    "DOWNLOAD",
    "DOWNLOADS",
    "BUILD",
    "BUILDING",
    "BUILD FROM SOURCE",
    "DEVELOPMENT",
    "SETUP",
    "LICENSE",
    "LICENSING",
    "CONTRIBUTING",
    "CHANGELOG",
    "RELEASES",
    "ACKNOWLEDGEMENTS",
    "ACKNOWLEDGMENTS",
    "CREDITS",
    "SPONSORS",
    "TABLE OF CONTENTS",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Family {
    Rust,
    Python,
    Script,
    Go,
    Managed,
    C,
    Ruby,
    Shell,
    Lua,
    Elixir,
}

impl Family {
    fn for_extension(extension: &str) -> Option<Self> {
        Some(match extension {
            "rs" => Self::Rust,
            "py" => Self::Python,
            "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" | "mts" | "cts" => Self::Script,
            "go" => Self::Go,
            "java" | "kt" | "kts" | "scala" | "swift" | "cs" | "dart" | "php" => Self::Managed,
            "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" => Self::C,
            "rb" => Self::Ruby,
            "sh" | "bash" => Self::Shell,
            "lua" => Self::Lua,
            "ex" | "exs" => Self::Elixir,
            _ => return None,
        })
    }

    fn hash_comments(self) -> bool {
        matches!(self, Self::Python | Self::Ruby | Self::Shell | Self::Elixir)
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn directory(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(directory, _)| directory)
}

fn extension(path: &str) -> String {
    file_name(path)
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default()
}

fn stem(path: &str) -> String {
    let name = file_name(path);
    name.split_once('.')
        .map_or(name, |(stem, _)| stem)
        .to_ascii_uppercase()
}

fn directories(path: &str) -> impl Iterator<Item = String> + '_ {
    directory(path)
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_ascii_lowercase)
}

fn in_directory_named(path: &str, names: &[&str]) -> bool {
    directories(path).any(|segment| names.contains(&segment.as_str()))
}

/// Hidden, vendored, generated, or build-output trees.
fn in_excluded_tree(path: &str) -> bool {
    directories(path).any(|segment| segment.starts_with('.'))
        || in_directory_named(path, &EXCLUDED_DIRECTORIES)
}

fn is_generated_or_minified(path: &str) -> bool {
    let name = file_name(path).to_ascii_lowercase();
    [
        ".min.",
        ".bundle.",
        ".generated.",
        "_generated.",
        ".pb.",
        "_pb2.",
        ".g.dart",
        ".d.ts",
    ]
    .iter()
    .any(|marker| name.contains(marker))
        || name.ends_with(".designer.cs")
}

fn is_test(path: &str) -> bool {
    let name = file_name(path).to_ascii_lowercase();
    in_directory_named(path, &TEST_DIRECTORIES)
        || name.starts_with("test_")
        || [".test.", ".spec.", "_test.", "_spec.", "tests."]
            .iter()
            .any(|marker| name.contains(marker))
        || name.ends_with("test.java")
        || name.ends_with("tests.java")
}

/// Reads a tracked regular file as text. Symlinks, oversized files, binary
/// files, and anything that is not UTF-8 are skipped, so a tracked link cannot
/// pull content from outside the repository into a prompt.
fn read_text(repo: &Path, relative: &str) -> Option<String> {
    let relative_path = Path::new(relative);
    if !relative_path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return None;
    }
    let path = repo.join(relative_path);
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_TEXT_BYTES {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    if bytes.contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn tracked_size(repo: &Path, relative: &str) -> Option<u64> {
    let metadata = std::fs::symlink_metadata(repo.join(relative)).ok()?;
    (metadata.is_file() && metadata.len() > 0 && metadata.len() <= MAX_TEXT_BYTES)
        .then_some(metadata.len())
}

/// Plain printable ASCII: common typographic symbols fold to ASCII, tabs
/// become spaces, and anything else outside ASCII is dropped.
fn ascii_line(line: &str) -> String {
    let mut ascii = String::with_capacity(line.len());
    for character in line.chars() {
        match character {
            '\u{2018}' | '\u{2019}' => ascii.push('\''),
            '\u{201C}' | '\u{201D}' => ascii.push('"'),
            '\u{2010}'..='\u{2015}' => ascii.push('-'),
            '\u{2026}' => ascii.push_str("..."),
            '\u{2192}' | '\u{21D2}' => ascii.push_str("->"),
            '\u{2190}' => ascii.push_str("<-"),
            '\u{2194}' => ascii.push_str("<->"),
            '\u{00D7}' => ascii.push('x'),
            '\u{2022}' => ascii.push('-'),
            '\t' => ascii.push(' '),
            ' '..='~' => ascii.push(character),
            _ => {}
        }
    }
    ascii.trim_end().to_string()
}

/// Redacts locations from excerpt text: anything shaped like a path, file
/// name, extension, or link, plus any word naming one of this repository's
/// own tracked files or directories.
#[derive(Default)]
struct Redactor {
    /// Tracked file names, matched exactly (`Makefile`, `logo.svg`).
    files: HashSet<String>,
    /// Lowercase tracked directory names, matched inside slashed words.
    directories: HashSet<String>,
}

impl Redactor {
    fn new(tracked: &[String]) -> Self {
        let mut redactor = Self::default();
        for path in tracked {
            let name = file_name(path);
            if name.len() >= 3 {
                redactor.files.insert(name.to_string());
            }
            redactor
                .directories
                .extend(directories(path).filter(|segment| segment.len() >= 2));
        }
        redactor
    }

    fn names_tracked_location(&self, word: &str) -> bool {
        let core = word
            .trim_matches(|c: char| {
                !c.is_ascii_alphanumeric() && !matches!(c, '.' | '/' | '_' | '-')
            })
            .trim_end_matches('.');
        self.files.contains(core)
            || (core.contains('/')
                && core.split('/').any(|segment| {
                    self.files.contains(segment)
                        || self.directories.contains(&segment.to_ascii_lowercase())
                }))
    }

    /// Replaces every location word with a placeholder.
    fn redact(&self, line: &str) -> String {
        let leading = line.len() - line.trim_start().len();
        let words = line
            .split_whitespace()
            .map(|word| {
                if word.contains("://") {
                    "[LINK]"
                } else if questions::is_location_word(word) || self.names_tracked_location(word) {
                    "[LOCATION]"
                } else {
                    word
                }
            })
            .collect::<Vec<_>>();
        format!("{}{}", &line[..leading], words.join(" "))
    }

    /// Printable ASCII with locations redacted.
    fn clean(&self, line: &str) -> String {
        self.redact(&ascii_line(line))
    }
}
/// Cuts `text` at a word boundary so it holds at most `max_chars` bytes.
fn clip(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }
    let mut end = max_chars;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let clipped = &text[..end];
    clipped
        .rfind(' ')
        .filter(|space| *space > max_chars / 2)
        .map_or(clipped, |space| &clipped[..space])
        .trim_end()
        .to_string()
}

/// Like [`clip`], but ends on a sentence when a complete one fits.
fn clip_sentences(text: &str, max_chars: usize) -> String {
    let clipped = clip(text, max_chars);
    if clipped.len() == text.len() {
        return clipped;
    }
    clipped
        .rfind(". ")
        .filter(|end| *end > max_chars / 3)
        .map_or(clipped.clone(), |end| clipped[..=end].to_string())
}

/// Joins whole lines while they fit in `budget` bytes.
fn fit_lines(lines: &[String], budget: usize) -> String {
    let mut text = String::new();
    for line in lines {
        let needed = line.len() + usize::from(!text.is_empty());
        if text.len() + needed > budget {
            if text.is_empty() {
                text = clip(line, budget);
            }
            break;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(line);
    }
    text
}

fn is_heading(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// [`fit_lines`] for documents: an excerpt never ends on a bare heading.
fn fit_document(lines: &[String], budget: usize) -> String {
    let mut text = fit_lines(lines, budget);
    while let Some(last) = text.lines().last() {
        if !is_heading(last) && !last.trim().is_empty() {
            break;
        }
        let end = text.len() - last.len();
        text.truncate(end);
        text.truncate(text.trim_end().len());
    }
    text
}

fn first_sentence(text: &str, max_chars: usize) -> String {
    let sentence = text.find(". ").map_or(text, |end| &text[..=end]).trim();
    clip(sentence, max_chars)
}

/// Removes link and image targets, keeping the link text.
fn strip_link_targets(line: &str) -> String {
    let mut text = String::new();
    let mut rest = line;
    while let Some(start) = rest.find("](") {
        text.push_str(&rest[..start]);
        rest = rest[start + 2..]
            .find(')')
            .map_or("", |end| &rest[start + 2 + end + 1..]);
    }
    text.push_str(rest);
    text.replace("![", "").replace(['[', ']'], "")
}

fn heading_level(trimmed: &str) -> Option<(usize, &str)> {
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    (level > 0).then(|| (level, trimmed[level..].trim()))
}

fn is_table_rule(trimmed: &str) -> bool {
    trimmed.starts_with('|')
        && trimmed
            .chars()
            .all(|character| matches!(character, '|' | '-' | ':' | ' '))
}

/// Readable prose lines: no front matter, code fences, raw HTML, badge-only
/// lines, table rules, link targets, or operational sections.
fn markdown_prose(text: &str, redactor: &Redactor) -> Vec<String> {
    let mut lines = Vec::new();
    let mut in_fence = false;
    let mut in_front_matter = text.starts_with("---");
    let mut skipped_section: Option<usize> = None;
    for (index, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        if in_front_matter {
            if index > 0 && trimmed == "---" {
                in_front_matter = false;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some((level, title)) = heading_level(trimmed) {
            if skipped_section.is_some_and(|skipped| level > skipped) {
                continue;
            }
            let title = title.trim_end_matches(':').to_ascii_uppercase();
            skipped_section = OPERATIONAL_SECTIONS
                .contains(&title.as_str())
                .then_some(level);
            if skipped_section.is_some() {
                continue;
            }
        } else if skipped_section.is_some() {
            continue;
        }
        if trimmed.starts_with('<')
            || trimmed.starts_with("[![")
            || trimmed.starts_with("![")
            || is_table_rule(trimmed)
        {
            continue;
        }
        let line = redactor.clean(&strip_link_targets(raw));
        if line.trim().is_empty() {
            if lines.last().is_some_and(|last: &String| !last.is_empty()) {
                lines.push(String::new());
            }
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// Headings plus the first paragraph under each (and before the first).
fn markdown_outline(text: &str, redactor: &Redactor) -> Vec<String> {
    let mut outline = Vec::new();
    let mut paragraph = String::new();
    let mut wants_paragraph = true;
    let flush = |paragraph: &mut String, outline: &mut Vec<String>| {
        if !paragraph.is_empty() {
            outline.push(clip_sentences(paragraph, PARAGRAPH_CHARS));
            paragraph.clear();
        }
    };
    for line in markdown_prose(text, redactor) {
        let trimmed = line.trim();
        if is_heading(trimmed) {
            flush(&mut paragraph, &mut outline);
            outline.push(trimmed.to_string());
            wants_paragraph = true;
        } else if trimmed.is_empty() {
            if !paragraph.is_empty() {
                flush(&mut paragraph, &mut outline);
                wants_paragraph = false;
            }
        } else if wants_paragraph && paragraph.len() < PARAGRAPH_CHARS * 2 {
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(trimmed);
        }
    }
    flush(&mut paragraph, &mut outline);
    outline
}

fn is_markdown(path: &str) -> bool {
    MARKDOWN_EXTENSIONS.contains(&extension(path).as_str())
}

fn readme(repo: &Path, tracked: &[String], redactor: &Redactor) -> Option<Vec<String>> {
    let mut candidates = tracked
        .iter()
        .filter(|path| !path.contains('/') && stem(path) == "README")
        .collect::<Vec<_>>();
    // Prefer Markdown, then the shortest name, then byte order.
    candidates.sort_by_key(|path| (!is_markdown(path), path.len(), path.as_str()));
    candidates.into_iter().find_map(|path| {
        let prose = markdown_prose(&read_text(repo, path)?, redactor);
        (!prose.is_empty()).then_some(prose)
    })
}

/// Design documents in reading priority: architecture-style documents, the
/// docs tree, decision records, contribution guides, then procedures.
fn design_documents(tracked: &[String]) -> Vec<&str> {
    let mut documents = tracked
        .iter()
        .map(String::as_str)
        .filter(|path| is_markdown(path) && !in_excluded_tree(path))
        .filter_map(|path| {
            let name = stem(path);
            let root_readme = !path.contains('/') && name == "README";
            if root_readme || NON_DESIGN_DOCUMENTS.contains(&name.as_str()) {
                return None;
            }
            let in_docs = matches!(directories(path).next().as_deref(), Some("docs" | "doc"));
            let priority = if DESIGN_DOCUMENTS.contains(&name.as_str()) {
                0
            } else if in_directory_named(path, &PROCEDURE_DIRECTORIES) {
                4
            } else if in_directory_named(path, &DECISION_DIRECTORIES) {
                2
            } else if in_docs {
                1
            } else if name == "CONTRIBUTING" {
                3
            } else {
                return None;
            };
            Some((priority, path.matches('/').count(), path))
        })
        .collect::<Vec<_>>();
    documents.sort_unstable();
    documents.into_iter().map(|(_, _, path)| path).collect()
}

fn cartridge_summary(repo: &Path, redactor: &Redactor) -> Option<String> {
    let source = read_text(repo, codequest::FILE_NAME)?;
    let config = toml::from_str::<toml::Value>(&source).ok()?;
    let summary = config.get("game")?.get("summary")?.as_str()?;
    let summary = redactor.clean(&summary.split_whitespace().collect::<Vec<_>>().join(" "));
    (!summary.is_empty()).then(|| clip(&summary, SUMMARY_BUDGET))
}

fn indent_of(line: &str) -> usize {
    line.chars()
        .take_while(|character| character.is_whitespace())
        .map(|character| if character == '\t' { 4 } else { 1 })
        .sum()
}

/// Tracks whether a scan is inside a `/* ... */` block, where continuation
/// lines start with `*` (and a leading `*` elsewhere is a dereference).
#[derive(Default)]
struct CommentScanner {
    in_block: bool,
}

impl CommentScanner {
    /// Text of a comment line with its markers removed, or `None` when the
    /// line is not a comment in this family.
    fn text<'a>(&mut self, trimmed: &'a str, family: Family) -> Option<&'a str> {
        let text = if family.hash_comments() {
            trimmed.strip_prefix('#')?
        } else if let Some(text) = ["///", "//!", "//"]
            .iter()
            .find_map(|marker| trimmed.strip_prefix(marker))
        {
            text
        } else if let Some(text) = ["/**", "/*"]
            .iter()
            .find_map(|marker| trimmed.strip_prefix(marker))
        {
            self.in_block = !text.contains("*/");
            text
        } else if self.in_block {
            self.in_block = !trimmed.contains("*/");
            trimmed
        } else {
            return None;
        };
        Some(text.trim_end_matches("*/").trim_start_matches('*').trim())
    }
}

fn is_annotation(trimmed: &str, family: Family) -> bool {
    match family {
        Family::Rust => trimmed.starts_with("#[") || trimmed.starts_with("#!["),
        Family::Python | Family::Script | Family::Managed => trimmed.starts_with('@'),
        _ => false,
    }
}

fn rust_signature(trimmed: &str, indent: usize) -> bool {
    let (public, mut item) = if let Some(rest) = trimmed.strip_prefix("pub(") {
        (true, rest.split_once(") ").map_or("", |(_, item)| item))
    } else if let Some(rest) = trimmed.strip_prefix("pub ") {
        (true, rest)
    } else {
        (false, trimmed)
    };
    while let Some(rest) = ["async ", "const ", "unsafe ", "extern \"C\" ", "default "]
        .iter()
        .find_map(|qualifier| item.strip_prefix(qualifier))
    {
        item = rest;
    }
    let kind = [
        "fn ", "struct ", "enum ", "trait ", "impl ", "impl<", "type ", "union ",
    ]
    .iter()
    .find(|keyword| item.starts_with(**keyword));
    match kind {
        Some(&"fn ") => indent == 0 || (public && indent <= 4),
        Some(_) => indent == 0,
        None => indent == 0 && item.starts_with("macro_rules!"),
    }
}

fn python_signature(trimmed: &str, indent: usize) -> bool {
    let definition = trimmed
        .strip_prefix("async ")
        .unwrap_or(trimmed)
        .strip_prefix("def ");
    match definition {
        Some(name) => {
            indent == 0 || (indent <= 4 && (!name.starts_with('_') || name.starts_with("__init__")))
        }
        None => indent == 0 && trimmed.starts_with("class "),
    }
}

fn script_signature(trimmed: &str, indent: usize) -> bool {
    if indent != 0 || trimmed.starts_with("export {") || trimmed.starts_with("export *") {
        return false;
    }
    let item = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    let item = item.strip_prefix("default ").unwrap_or(item);
    if ["const ", "let ", "var "]
        .iter()
        .any(|keyword| item.starts_with(keyword))
    {
        // Only bindings that define behavior; plain values are constants.
        return item.contains("=>") || item.contains("function") || item.contains("class ");
    }
    trimmed.starts_with("export ")
        || [
            "function ",
            "async function ",
            "class ",
            "abstract class ",
            "interface ",
            "type ",
            "enum ",
            "declare ",
        ]
        .iter()
        .any(|keyword| item.starts_with(keyword))
}

fn managed_signature(trimmed: &str, indent: usize) -> bool {
    const MODIFIERS: [&str; 16] = [
        "public",
        "protected",
        "internal",
        "open",
        "abstract",
        "sealed",
        "final",
        "static",
        "override",
        "data",
        "inline",
        "suspend",
        "async",
        "partial",
        "readonly",
        "export",
    ];
    const TYPES: [&str; 10] = [
        "class",
        "interface",
        "enum",
        "record",
        "struct",
        "object",
        "protocol",
        "extension",
        "trait",
        "actor",
    ];
    const FUNCTIONS: [&str; 4] = ["fun", "func", "def", "function"];
    if indent > 4 || trimmed.ends_with(';') {
        return false;
    }
    let mut words = trimmed.split_whitespace().peekable();
    let mut modified = false;
    while let Some(word) = words.peek() {
        if *word == "private" || *word == "fileprivate" {
            return false;
        }
        if !MODIFIERS.contains(word) {
            break;
        }
        modified = true;
        words.next();
    }
    let Some(word) = words.next() else {
        return false;
    };
    TYPES.contains(&word)
        || FUNCTIONS.contains(&word)
        || (modified && trimmed.contains('(') && !trimmed.contains(" = "))
}

fn c_signature(trimmed: &str, indent: usize) -> bool {
    const CONTROL: [&str; 8] = [
        "return", "if", "for", "while", "else", "switch", "case", "do",
    ];
    if indent != 0 || !trimmed.starts_with(|character: char| character.is_ascii_alphabetic()) {
        return false;
    }
    if [
        "struct ",
        "class ",
        "enum ",
        "typedef ",
        "namespace ",
        "union ",
        "template",
    ]
    .iter()
    .any(|keyword| trimmed.starts_with(keyword))
    {
        return true;
    }
    let first = trimmed
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .next()
        .unwrap_or_default();
    let before_call = trimmed.split('(').next().unwrap_or_default();
    trimmed.contains('(') && !CONTROL.contains(&first) && !before_call.contains('=')
}

fn is_signature(line: &str, family: Family) -> bool {
    let indent = indent_of(line);
    let trimmed = line.trim();
    match family {
        Family::Rust => rust_signature(trimmed, indent),
        Family::Python => python_signature(trimmed, indent),
        Family::Script => script_signature(trimmed, indent),
        Family::Go => indent == 0 && (trimmed.starts_with("func ") || trimmed.starts_with("type ")),
        Family::Managed => managed_signature(trimmed, indent),
        Family::C => c_signature(trimmed, indent),
        Family::Ruby => {
            indent <= 2
                && ["def ", "class ", "module "]
                    .iter()
                    .any(|keyword| trimmed.starts_with(keyword))
        }
        Family::Shell => {
            indent == 0
                && (trimmed.starts_with("function ")
                    || trimmed.split_once("()").is_some_and(|(name, _)| {
                        let name = name.trim();
                        !name.is_empty()
                            && name
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
                    }))
        }
        Family::Lua => {
            indent == 0
                && (trimmed.starts_with("function ") || trimmed.starts_with("local function "))
        }
        Family::Elixir => {
            (indent == 0 && trimmed.starts_with("defmodule "))
                || (indent <= 2
                    && ["def ", "defprotocol ", "defstruct "]
                        .iter()
                        .any(|keyword| trimmed.starts_with(keyword)))
        }
    }
}

/// Whether the signature exposes the item outside its file or module.
fn is_public(signature: &str, family: Family) -> bool {
    match family {
        Family::Rust => signature.starts_with("pub"),
        Family::Script => signature.starts_with("export"),
        Family::Managed => ["public", "open", "internal", "export"]
            .iter()
            .any(|modifier| signature.starts_with(modifier)),
        Family::Python => !signature
            .trim_start_matches("async ")
            .trim_start_matches("def ")
            .starts_with('_'),
        _ => true,
    }
}

/// Byte offset of the first `stop` character outside brackets.
fn outside_brackets(text: &str, stop: char) -> Option<usize> {
    let mut depth = 0usize;
    for (index, character) in text.char_indices() {
        match character {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' | '>' => depth = depth.saturating_sub(1),
            _ if character == stop && depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

/// A signature without its body: cut at the body's opening brace (or a
/// Python colon, or a Ruby/Elixir `do`) and clipped to one compact line.
fn signature_text(header: &str, family: Family, redactor: &Redactor) -> String {
    let stop = if family == Family::Python { ':' } else { '{' };
    // `->` is not a closing bracket, so hide its `>` from the depth count.
    let masked = header.replace("->", "-~").replace("=>", "=~");
    let head = outside_brackets(&masked, stop).map_or(header, |end| &header[..end]);
    let head = head.trim_end();
    let head = head.strip_suffix(" do").unwrap_or(head).trim_end();
    clip(&redactor.clean(head), SIGNATURE_CHARS)
}

/// Joins a signature that continues across lines (open parentheses and no
/// body yet) into one header.
fn signature_header(lines: &[&str], start: usize) -> String {
    let mut header = lines[start].trim().to_string();
    let balance =
        |text: &str| text.matches('(').count() as isize - text.matches(')').count() as isize;
    for next in lines.iter().skip(start + 1).take(MAX_SIGNATURE_LINES) {
        if balance(&header) <= 0 || outside_brackets(&header, '{').is_some() {
            break;
        }
        let next = next.trim();
        if !header.ends_with('(') && !next.starts_with(')') {
            header.push(' ');
        }
        header.push_str(next);
    }
    header.replace(",)", ")")
}

/// The file-level description: Rust inner doc comments, a Python module
/// docstring, or the leading comment block, unless it is a license header.
fn file_about(lines: &[&str], family: Family, redactor: &Redactor) -> Option<String> {
    let mut about = Vec::new();
    let mut in_docstring = false;
    let mut comments = CommentScanner::default();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if in_docstring {
            let (text, closed) = match trimmed.find("\"\"\"") {
                Some(end) => (&trimmed[..end], true),
                None => (trimmed, false),
            };
            about.push(text.to_string());
            if closed {
                break;
            }
            continue;
        }
        if trimmed.is_empty()
            || (index == 0 && trimmed.starts_with("#!"))
            || trimmed.starts_with("#![")
            || trimmed.contains("coding:")
            || trimmed == "\"use strict\";"
            || trimmed == "'use strict';"
        {
            if about.is_empty() {
                continue;
            }
            break;
        }
        if family == Family::Python && about.is_empty() && trimmed.starts_with("\"\"\"") {
            let body = &trimmed[3..];
            match body.find("\"\"\"") {
                Some(end) => {
                    about.push(body[..end].to_string());
                    break;
                }
                None => {
                    about.push(body.to_string());
                    in_docstring = true;
                    continue;
                }
            }
        }
        let text = if family == Family::Rust {
            trimmed.strip_prefix("//!").map(str::trim)
        } else {
            comments.text(trimmed, family)
        };
        match text {
            Some(text) => about.push(text.to_string()),
            None => break,
        }
    }
    let about = about
        .iter()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let lowercase = about.to_ascii_lowercase();
    if about.is_empty()
        || ["copyright", "license", "spdx"]
            .iter()
            .any(|marker| lowercase.contains(marker))
    {
        return None;
    }
    Some(clip_sentences(&redactor.clean(&about), ABOUT_CHARS))
}

/// One skeleton entry: the file description, or a signature with its doc.
struct SkeletonEntry {
    /// Lower ranks survive a tight budget first: the description, then
    /// documented public items, public items, documented private items, and
    /// everything else.
    rank: u8,
    lines: Vec<String>,
}

impl SkeletonEntry {
    fn len(&self) -> usize {
        self.lines.iter().map(|line| line.len() + 1).sum()
    }
}

/// Doc comments and top-level signatures, in source order.
fn skeleton(text: &str, family: Family, redactor: &Redactor) -> Vec<SkeletonEntry> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut skeleton = Vec::new();
    if let Some(about) = file_about(&lines, family, redactor) {
        skeleton.push(SkeletonEntry {
            rank: 0,
            lines: vec![format!("ABOUT: {about}")],
        });
    }
    let mut comment = Vec::new();
    let mut comments = CommentScanner::default();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            comment.clear();
            continue;
        }
        if is_annotation(trimmed, family) || trimmed.starts_with("//!") {
            continue;
        }
        if let Some(text) = comments.text(trimmed, family) {
            if indent_of(line) <= 4 && !text.is_empty() {
                comment.push(text.to_string());
            }
            continue;
        }
        if is_signature(line, family) {
            let mut doc = comment.join(" ");
            if doc.is_empty() && family == Family::Python {
                doc = lines[index + 1..]
                    .iter()
                    .map(|next| next.trim())
                    .find(|next| !next.is_empty())
                    .and_then(|next| next.strip_prefix("\"\"\""))
                    .map(|next| next.trim_end_matches("\"\"\"").to_string())
                    .unwrap_or_default();
            }
            let doc = redactor.clean(&first_sentence(doc.trim(), DOC_SENTENCE_CHARS));
            let signature = signature_text(&signature_header(&lines, index), family, redactor);
            if !signature.is_empty() {
                let public = is_public(&signature, family);
                let rank = match (public, doc.is_empty()) {
                    (true, false) => 1,
                    (true, true) => 2,
                    (false, false) => 3,
                    (false, true) => 4,
                };
                let mut entry = Vec::new();
                if !doc.is_empty() {
                    entry.push(format!("// {doc}"));
                }
                entry.push(signature);
                skeleton.push(SkeletonEntry { rank, lines: entry });
            }
        }
        comment.clear();
    }
    skeleton
}

/// The most informative entries that fit in `budget` bytes, in source order.
fn fit_skeleton(skeleton: &[SkeletonEntry], budget: usize) -> Vec<String> {
    let mut by_rank = (0..skeleton.len()).collect::<Vec<_>>();
    by_rank.sort_by_key(|index| (skeleton[*index].rank, *index));
    let mut used = 0;
    let mut kept = Vec::new();
    for index in by_rank {
        let length = skeleton[index].len();
        if used + length <= budget {
            used += length;
            kept.push(index);
        }
    }
    kept.sort_unstable();
    kept.into_iter()
        .flat_map(|index| skeleton[index].lines.iter().cloned())
        .collect()
}

fn source_family(path: &str) -> Option<Family> {
    if in_excluded_tree(path) || is_generated_or_minified(path) || is_test(path) {
        return None;
    }
    Family::for_extension(&extension(path))
}

/// Source files in an order that spreads across the tree: one file from each
/// directory per round, largest first within a directory. The project's own
/// directories come before tooling, packaging, samples, and shell scripts.
fn spread_sources<'a>(repo: &Path, tracked: &'a [String]) -> Vec<(bool, &'a str, Family)> {
    type Files<'p> = Vec<(std::cmp::Reverse<u64>, &'p str, Family)>;
    let mut by_directory = BTreeMap::<(bool, &str), Files<'a>>::new();
    for path in tracked {
        let Some(family) = source_family(path) else {
            continue;
        };
        let Some(size) = tracked_size(repo, path) else {
            continue;
        };
        let auxiliary = family == Family::Shell || in_directory_named(path, &AUXILIARY_DIRECTORIES);
        by_directory
            .entry((auxiliary, directory(path)))
            .or_default()
            .push((std::cmp::Reverse(size), path.as_str(), family));
    }
    for files in by_directory.values_mut() {
        files.sort_unstable();
    }
    let mut spread = Vec::new();
    for auxiliary in [false, true] {
        let mut groups = by_directory
            .iter()
            .filter(|((group_is_auxiliary, _), _)| *group_is_auxiliary == auxiliary)
            .map(|(_, files)| files.iter())
            .collect::<Vec<_>>();
        loop {
            let before = spread.len();
            for group in &mut groups {
                if let Some((_, path, family)) = group.next() {
                    spread.push((auxiliary, *path, *family));
                }
            }
            if spread.len() == before {
                break;
            }
        }
    }
    spread
}

fn component_skeletons(
    repo: &Path,
    tracked: &[String],
    redactor: &Redactor,
) -> Vec<Vec<SkeletonEntry>> {
    let mut components = spread_sources(repo, tracked)
        .into_iter()
        .take(MAX_COMPONENT_READS)
        .filter_map(|(auxiliary, path, family)| {
            let skeleton = skeleton(&read_text(repo, path)?, family, redactor);
            (!skeleton.is_empty()).then_some(((auxiliary, path), skeleton))
        })
        .take(MAX_COMPONENTS)
        .collect::<Vec<_>>();
    // Number the project's own components first, each group in tree order so
    // related code stays adjacent; the labels reveal nothing about locations.
    components.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    components
        .into_iter()
        .map(|(_, skeleton)| skeleton)
        .collect()
}

/// Splits `total` bytes among components: each gets what it needs up to
/// [`COMPONENT_BUDGET`], and what small components leave unused flows to
/// larger ones.
fn component_allocations(needs: &[usize], total: usize) -> Vec<usize> {
    let mut order = (0..needs.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| (needs[*index], *index));
    let mut allocations = vec![0; needs.len()];
    let mut remaining = total;
    for (served, index) in order.into_iter().enumerate() {
        let share = remaining / (needs.len() - served);
        allocations[index] = needs[index].min(share).min(COMPONENT_BUDGET);
        remaining -= allocations[index];
    }
    allocations
}

struct Brief {
    text: String,
}

impl Brief {
    fn remaining(&self) -> usize {
        BRIEF_BUDGET.saturating_sub(self.text.len())
    }

    /// Appends `heading` and `body`, or nothing when the body is empty.
    fn section(&mut self, heading: &str, body: &str) -> usize {
        if body.trim().is_empty() {
            return 0;
        }
        let before = self.text.len();
        if !self.text.is_empty() {
            self.text.push_str("\n\n");
        }
        self.text.push_str(heading);
        self.text.push('\n');
        self.text.push_str(body);
        self.text.len() - before
    }

    /// Bytes a section body may use under `budget` and the whole brief.
    fn body_budget(&self, heading: &str, budget: usize) -> usize {
        let overhead = heading.len() + 1 + if self.text.is_empty() { 0 } else { 2 };
        budget.min(self.remaining().saturating_sub(overhead))
    }
}

/// The project brief for `repo`, built from its tracked files (paths relative
/// to the repository root, `/`-separated, as `git ls-files` reports them).
pub(crate) fn project_brief(repo: &Path, tracked: &[String]) -> String {
    let redactor = Redactor::new(tracked);
    let mut brief = Brief {
        text: String::new(),
    };
    if let Some(summary) = cartridge_summary(repo, &redactor) {
        let heading = "CARTRIDGE SUMMARY:";
        let body = clip(&summary, brief.body_budget(heading, SUMMARY_BUDGET));
        brief.section(heading, &body);
    }
    if let Some(readme) = readme(repo, tracked, &redactor) {
        let heading = "README EXCERPT:";
        let body = fit_document(&readme, brief.body_budget(heading, README_BUDGET));
        brief.section(heading, &body);
    }
    let mut notes_budget = DESIGN_NOTES_BUDGET;
    let mut note = 0;
    for path in design_documents(tracked) {
        if notes_budget < MIN_NOTE_BUDGET {
            break;
        }
        let Some(outline) = read_text(repo, path).map(|text| markdown_outline(&text, &redactor))
        else {
            continue;
        };
        let heading = format!("DESIGN NOTE {}:", note + 1);
        let budget = brief.body_budget(&heading, DESIGN_NOTE_BUDGET.min(notes_budget));
        let used = brief.section(&heading, &fit_document(&outline, budget));
        if used > 0 {
            note += 1;
            notes_budget = notes_budget.saturating_sub(used);
        }
    }

    let components = component_skeletons(repo, tracked, &redactor);
    let heading =
        "COMPONENT SKELETONS (doc comments and top-level signatures; locations withheld):";
    let available = brief.body_budget(heading, BRIEF_BUDGET);
    let count = components.len().min(available / MIN_COMPONENT_BUDGET);
    let labels = (1..=count)
        .map(|number| format!("--- COMPONENT {number} ---"))
        .collect::<Vec<_>>();
    let needs = components
        .iter()
        .zip(&labels)
        .map(|(skeleton, label)| {
            label.len() + 1 + skeleton.iter().map(SkeletonEntry::len).sum::<usize>()
        })
        .collect::<Vec<_>>();
    let mut body = Vec::new();
    for ((skeleton, label), allocation) in components
        .iter()
        .zip(&labels)
        .zip(component_allocations(&needs, available))
    {
        let lines = fit_skeleton(skeleton, allocation.saturating_sub(label.len() + 1));
        if !lines.is_empty() {
            body.push(label.clone());
            body.extend(lines);
        }
    }
    brief.section(heading, &fit_lines(&body, available));

    if brief.text.is_empty() {
        return "(no documentation or source excerpts available)".to_string();
    }
    brief.text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_tools;

    fn temporary_git_repo() -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "codequest-context-test-{}-{unique}.cartridge",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        let status = external_tools::git_command()
            .arg("-C")
            .arg(&path)
            .args(["init", "--quiet"])
            .status()
            .unwrap();
        assert!(status.success());
        path
    }

    fn write(repo: &Path, relative: &str, contents: impl AsRef<[u8]>) {
        let path = repo.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn tracked_files(repo: &Path) -> Vec<String> {
        let add = external_tools::git_command()
            .arg("-C")
            .arg(repo)
            .args(["add", "--all", "--force"])
            .status()
            .unwrap();
        assert!(add.success());
        let output = external_tools::git_command()
            .arg("-C")
            .arg(repo)
            .args(["ls-files", "-z"])
            .output()
            .unwrap();
        String::from_utf8(output.stdout)
            .unwrap()
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn sample_repo() -> std::path::PathBuf {
        let repo = temporary_git_repo();
        write(
            &repo,
            "README.md",
            "# Tide Clock\n\n[![build](https://ci.example/badge.svg)](https://ci.example)\n\nTide Clock predicts harbor tides from lunar phase so sailors can plan departures.\nSee [the guide](docs/guide.md) or run `scripts/setup.sh`.\n\n```sh\nnpm install tide-clock\n```\n\n## Installation\n\nDownload the installer for your platform.\n\n### From source\n\nClone and build it yourself.\n\n## Why\n\n| Mode | Meaning |\n|---|---|\n| Live | Observed \u{2192} predicted |\n\nPredictions must never contradict an observed tide.\n\n## Roadmap\n",
        );
        write(
            &repo,
            "docs/architecture.md",
            "---\ntitle: internals\n---\n# Architecture\n\nThe predictor is pure; the scheduler owns every clock.\n\nA second paragraph that the outline leaves out.\n\n## Observations\n\nObserved tides override predictions until the next cycle.\n",
        );
        write(
            &repo,
            "docs/runbooks/rotate-keys.md",
            "# Rotating keys\n\nRevoke, reissue, and redeploy.\n",
        );
        write(
            &repo,
            "CHANGELOG.md",
            "# Changelog\n\n## 1.2.3\n\nFixed a bug.\n",
        );
        write(
            &repo,
            codequest::FILE_NAME,
            "schema_version = 1\n\n[game]\ntype = \"quiz\"\nsummary = \"Learn how the tide model stays honest.\"\n",
        );
        write(
            &repo,
            "src/predictor.rs",
            "//! Pure tide prediction from lunar phase.\n//! Configured by config/tides.toml at startup.\n\nuse std::f64::consts::PI;\nmod harmonics;\n\nconst CYCLE: f64 = 12.42;\n\n/// Predicts the tide height. Never reads the clock.\n#[must_use]\npub fn predict(phase: f64) -> f64 {\n    phase.sin() * PI\n}\n\npub struct Tide {\n    height: f64,\n}\n\nimpl Tide {\n    /// Applies an observation.\n    pub fn observe(&mut self, height: f64) {\n        *self.last() = height;\n    }\n    fn secret(&self) {}\n}\n\n/// Blends two readings.\npub fn blend(\n    first: f64,\n    second: f64,\n) -> f64 {\n    first + second\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn predicts() {}\n}\n",
        );
        write(
            &repo,
            "app/scheduler.py",
            "\"\"\"Owns every clock and decides when predictions refresh.\"\"\"\n\nimport time\n\nclass Scheduler:\n    def tick(self, now):\n        \"\"\"Advances the schedule by one step.\"\"\"\n        return now\n\n    def _internal(self):\n        pass\n\ndef shortcut(value): return value\n",
        );
        write(
            &repo,
            "web/view.ts",
            "// Copyright (c) 2024 Someone. MIT License.\n\nimport { predict } from './predictor';\n\nexport const REFRESH_SECONDS = 30;\n\n/** Renders the next tide for sailors. */\nexport function renderTide({ height }: Reading): string {\n  return `${height}`;\n}\n\nconst helper = 3;\n",
        );
        write(
            &repo,
            "node_modules/leftpad/index.js",
            "export function vendoredSecret() {}\n",
        );
        write(
            &repo,
            "web/app.min.js",
            "export function minifiedSecret(){}\n",
        );
        write(
            &repo,
            "web/view.test.ts",
            "export function testSecret() {}\n",
        );
        write(&repo, "Cargo.lock", "# generated lockSecret\n");
        write(&repo, "src/blob.rs", b"pub fn binarySecret() {}\0\x01\x02");
        write(
            &repo,
            "src/huge.rs",
            format!(
                "pub fn hugeSecret() {{}}\n{}",
                "// padding\n".repeat(60_000)
            ),
        );
        repo
    }

    #[test]
    fn brief_covers_docs_summary_and_component_skeletons_without_locations() {
        let repo = sample_repo();
        let tracked = tracked_files(&repo);

        let brief = project_brief(&repo, &tracked);

        assert!(brief.contains("CARTRIDGE SUMMARY:\nLearn how the tide model stays honest."));
        assert!(brief.contains("README EXCERPT:\n# Tide Clock"));
        assert!(brief.contains("predicts harbor tides from lunar phase"));
        assert!(brief.contains("| Live | Observed -> predicted |"));
        assert!(brief.contains("Predictions must never contradict an observed tide."));
        assert!(
            !brief.contains("Roadmap"),
            "excerpts never end on a bare heading"
        );
        assert!(!brief.contains("|---|"));
        for operational in [
            "Installation",
            "installer",
            "From source",
            "Clone and build",
        ] {
            assert!(!brief.contains(operational), "{operational} leaked");
        }
        assert!(brief.contains("DESIGN NOTE 1:\n# Architecture\nThe predictor is pure"));
        assert!(brief.contains("## Observations\nObserved tides override predictions"));
        assert!(
            brief.contains("DESIGN NOTE 2:\n# Rotating keys"),
            "procedures come last"
        );
        assert!(!brief.contains("second paragraph"));
        assert!(!brief.contains("Fixed a bug"), "changelogs are not design");

        assert!(brief.contains("--- COMPONENT 1 ---"));
        assert!(brief.contains("ABOUT: Pure tide prediction from lunar phase."));
        assert!(brief.contains("// Predicts the tide height.\npub fn predict(phase: f64) -> f64\n"));
        assert!(brief.contains("pub struct Tide"));
        assert!(
            brief.contains("// Applies an observation.\npub fn observe(&mut self, height: f64)\n")
        );
        assert!(brief
            .contains("// Blends two readings.\npub fn blend(first: f64, second: f64) -> f64\n"));
        assert!(brief.contains("ABOUT: Owns every clock and decides when predictions refresh."));
        assert!(brief.contains("class Scheduler\n"));
        assert!(brief.contains("// Advances the schedule by one step.\ndef tick(self, now)\n"));
        assert!(brief.contains("def shortcut(value)"));
        assert!(brief.contains(
            "// Renders the next tide for sailors.\nexport function renderTide({ height }: Reading): string"
        ));
        for hidden in [
            "secret",
            "Secret",
            "use std",
            "mod harmonics",
            "CYCLE",
            "REFRESH_SECONDS",
            "helper",
            "fn predicts",
            "Copyright",
            "npm install",
            "import ",
            "return value",
            "self.last",
        ] {
            assert!(!brief.contains(hidden), "{hidden} leaked into:\n{brief}");
        }
        for location in [
            "predictor.rs",
            "scheduler.py",
            "view.ts",
            "src/",
            "app/",
            "docs/",
            "guide.md",
            "setup.sh",
            "tides.toml",
            "node_modules",
            "ci.example",
            "README.md",
            "runbooks",
        ] {
            assert!(
                !brief.contains(location),
                "{location} leaked into:\n{brief}"
            );
        }
        assert!(
            brief.contains("[LOCATION]"),
            "path mentions are redacted in place"
        );
        assert!(brief.is_ascii());
        assert!(brief.len() <= BRIEF_BUDGET);

        std::fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn brief_is_deterministic_and_components_spread_across_the_tree() {
        let repo = temporary_git_repo();
        for directory in ["alpha", "beta", "gamma"] {
            for index in 0..20 {
                write(
                    &repo,
                    &format!("{directory}/module_{index:02}.rs"),
                    format!(
                        "/// Part {index} of {directory}.\npub fn {directory}_part_{index}() {{}}\n{}",
                        "// body\n".repeat(index)
                    ),
                );
            }
        }
        for index in 0..5 {
            write(
                &repo,
                &format!("scripts/tool_{index}.py"),
                format!("def tooling_{index}():\n    pass\n"),
            );
        }
        let tracked = tracked_files(&repo);

        let brief = project_brief(&repo, &tracked);

        assert_eq!(brief, project_brief(&repo, &tracked));
        assert!(brief.contains(&format!("--- COMPONENT {MAX_COMPONENTS} ---")));
        assert!(!brief.contains(&format!("--- COMPONENT {} ---", MAX_COMPONENTS + 1)));
        for directory in ["alpha", "beta", "gamma"] {
            let picked = brief.matches(&format!("pub fn {directory}_part_")).count();
            assert_eq!(picked, MAX_COMPONENTS / 3, "{directory}");
            assert!(
                brief.contains(&format!("pub fn {directory}_part_19()")),
                "the largest file in each directory is picked first"
            );
        }
        assert!(
            !brief.contains("tooling_"),
            "tooling waits until the project's own components run out"
        );
        assert!(brief.len() <= BRIEF_BUDGET);

        std::fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn large_components_get_the_budget_small_ones_leave_and_keep_public_items() {
        assert_eq!(
            component_allocations(&[100, 5000, 200], 3000),
            [100, 1600, 200]
        );
        assert_eq!(
            component_allocations(&[900, 900, 900], 1500),
            [500, 500, 500]
        );

        let mut source = String::new();
        for index in 0..40 {
            source.push_str(&format!("fn private_helper_{index}() {{}}\n"));
        }
        source.push_str("/// The entry point.\npub fn entry_point() {}\n");
        let skeleton = skeleton(&source, Family::Rust, &Redactor::default());
        let fitted = fit_skeleton(&skeleton, 200);
        assert_eq!(
            fitted[fitted.len() - 2..],
            ["// The entry point.", "pub fn entry_point()"],
            "documented public items survive, still in source order"
        );
        assert!(fitted.iter().map(|line| line.len() + 1).sum::<usize>() <= 200);
        assert_eq!(
            fitted[0], "fn private_helper_0()",
            "leftover room holds private items"
        );
    }

    #[test]
    fn oversized_documentation_stays_within_the_brief_budget() {
        let repo = temporary_git_repo();
        let paragraph = "The engine owns every rule and the shell only draws. ".repeat(4);
        write(
            &repo,
            "README.md",
            format!("# Big\n\n{}", format!("{paragraph}\n\n").repeat(400)),
        );
        for index in 0..12 {
            write(
                &repo,
                &format!("docs/note-{index:02}.md"),
                format!("# Note {index}\n\n{paragraph}\n\n").repeat(30),
            );
        }
        for index in 0..40 {
            write(
                &repo,
                &format!("src/part_{index:02}/mod.rs"),
                format!(
                    "//! Part {index}.\n{}",
                    "pub fn a_rather_long_function_name_for_budgeting() {}\n".repeat(60)
                ),
            );
        }
        let tracked = tracked_files(&repo);

        let brief = project_brief(&repo, &tracked);

        assert!(brief.len() <= BRIEF_BUDGET, "{}", brief.len());
        let readme_end = brief.find("DESIGN NOTE 1:").unwrap();
        assert!(
            readme_end <= README_BUDGET + 64,
            "the README keeps to its share"
        );
        let notes = &brief[readme_end..brief.find("COMPONENT SKELETONS").unwrap()];
        assert!(notes.len() <= DESIGN_NOTES_BUDGET + 64, "{}", notes.len());
        assert!(
            brief.contains("--- COMPONENT 1 ---"),
            "components still get room"
        );

        std::fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn an_empty_repository_yields_a_placeholder_brief() {
        let repo = temporary_git_repo();
        assert_eq!(
            project_brief(&repo, &[]),
            "(no documentation or source excerpts available)"
        );
        std::fs::remove_dir_all(repo).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn tracked_symlinks_are_never_followed() {
        let repo = temporary_git_repo();
        let outside = repo.with_extension("outside.rs");
        std::fs::write(&outside, "pub fn outsideSecret() {}\n").unwrap();
        std::os::unix::fs::symlink(&outside, repo.join("linked.rs")).unwrap();
        write(&repo, "README.md", "# Linked\n\nA project.\n");
        let tracked = tracked_files(&repo);
        assert!(tracked.contains(&"linked.rs".to_string()));

        let brief = project_brief(&repo, &tracked);

        assert!(!brief.contains("outsideSecret"));
        std::fs::remove_file(outside).unwrap();
        std::fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn redaction_keeps_prose_and_replaces_locations() {
        let redactor = Redactor::new(&[
            "packaging/Containerfile".to_string(),
            "Makefile".to_string(),
            "assets/logo.svg".to_string(),
        ]);
        assert_eq!(
            redactor.redact("See packaging/Containerfile, the Makefile, and logo.svg."),
            "See [LOCATION] the [LOCATION] and [LOCATION]"
        );
        assert_eq!(
            redactor.redact("Builds on Fedora/Arch, not in the packaging stage."),
            "Builds on Fedora/Arch, not in the packaging stage."
        );
        assert_eq!(
            redactor.redact("  Read src/engine.rs, then see https://example.com and main.c."),
            "  Read [LOCATION] then see [LINK] and [LOCATION]"
        );
        assert_eq!(
            redactor.redact("A read/write lock guards e.g. the save; see Node.js."),
            "A read/write lock guards e.g. the save; see Node.js."
        );
        assert_eq!(redactor.redact("let ratio = a / b;"), "let ratio = a / b;");
    }
}
