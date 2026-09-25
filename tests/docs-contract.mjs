import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";

// The design brief, the designer skill, and the README describe engine
// behavior in prose. These checks tie each described fact to the source it
// comes from, so a change to one without the other fails here.

const read = (path) =>
  readFileSync(new URL(`../${path}`, import.meta.url), "utf8").replaceAll("\r\n", "\n");
// Prose wraps anywhere, so phrases are matched with whitespace collapsed.
const prose = (path) => read(path).replace(/\s+/g, " ");

const rustFiles = readdirSync(new URL("../src-tauri/src/", import.meta.url), {
  recursive: true,
})
  .filter((name) => name.endsWith(".rs"))
  .map((name) => `src-tauri/src/${name.replaceAll("\\", "/")}`);
const rust = rustFiles.map((path) => [path, read(path)]);
const allRust = rust.map(([, source]) => source).join("\n");

function constant(name) {
  const value = allRust.match(new RegExp(`const ${name}: [^=]+= ([^;]+);`))?.[1];
  assert.ok(value, `Missing constant ${name}`);
  return value;
}

const brief = prose("docs/game-design/oracle-quiz.md");
const skill = prose(".agents/skills/codequest-game-designer/SKILL.md");
const polish = prose(".agents/skills/codequest-game-designer/references/polish.md");
const reference = prose("docs/reference/codequest-toml.md");
const readme = prose("README.md");
const manifests = ["CODEQUEST.toml", "docs/examples/CODEQUEST.toml"];

/* ---------- Every preview-writing test is listed where designers look ---------- */

const previewWriters = new Set();
for (const [, source] of rust) {
  let test = null;
  for (const line of source.split("\n")) {
    const declared = line.match(/^ {4}fn ([a-z0-9_]+)\(/);
    if (declared) test = declared[1];
    if (/maybe_write_preview\(/.test(line) && !/fn maybe_write_preview/.test(line)) {
      assert.ok(test, "A preview is written outside any function");
      previewWriters.add(test);
    }
  }
}
assert.ok(previewWriters.size > 0, "No test writes CQA_VISUAL_PREVIEW_DIR frames");
for (const test of previewWriters) {
  // Named inline or in the command block.
  assert.match(reference, new RegExp(`\\b${test}\\b`), `codequest-toml.md omits preview writer ${test}`);
  assert.match(skill, new RegExp(`\\b${test}\\b`), `SKILL.md omits preview writer ${test}`);
}

/* ---------- The spaced-retry distance matches RETRY_GAP ---------- */

const words = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];
const gap = words[Number(constant("RETRY_GAP"))];
assert.ok(gap, "RETRY_GAP needs a spelled-out number in these checks");
const retryPhrase = `up to ${gap} intervening questions`;
for (const [path, text] of [
  ["README.md", readme],
  ...manifests.map((path) => [path, prose(path)]),
  ["SKILL.md", skill],
  ["oracle-quiz.md", brief],
]) {
  assert.ok(text.includes(retryPhrase), `${path} must say a miss returns ${retryPhrase}`);
  assert.doesNotMatch(text, /questions later/, `${path} states a fixed retry distance`);
}

/* ---------- The generation repair pass is described ---------- */

const minRepair = constant("MIN_REPAIR_TIME").match(/from_secs\((\d+)\)/)?.[1];
assert.ok(minRepair, "MIN_REPAIR_TIME should be whole seconds");
const generationRow = brief.match(/\| Question generation \|[^\n]*?\| (?=\| )/)?.[0] || "";
assert.ok(generationRow, "The brief needs its Question generation traceability row");
for (const [path, text] of [
  ["oracle-quiz.md Question generation row", generationRow],
  ["README.md", readme],
]) {
  assert.match(text, /one (bounded )?repair call/, `${path} must describe the repair call`);
  assert.ok(text.includes(`${minRepair} seconds remain`), `${path} must give the repair floor`);
  assert.match(text, /at most two CLI calls/, `${path} must give the provider cost`);
}
assert.match(skill, /one bounded repair call/, "SKILL.md must say mechanical rejections get a repair");

/* ---------- The Oracle line's copy, categories, and cadence ---------- */

const reasonSource = allRust.match(/fn question_failure_reason\([\s\S]*?\n\}\n/)?.[0] || "";
assert.ok(reasonSource, "Missing question_failure_reason");
const reasons = [...reasonSource.matchAll(/^\s*"([A-Z][A-Z ]+)"\s*$/gm)].map((match) => match[1]);
assert.ok(reasons.length > 3, "question_failure_reason should return its categories");
for (const extra of ["AI DISABLED", "NO NEW QUESTIONS"]) {
  if (allRust.includes(`"${extra}"`)) reasons.push(extra);
}
for (const reason of reasons) {
  assert.ok(brief.includes(`\`${reason}\``), `oracle-quiz.md omits the Oracle reason ${reason}`);
}
assert.match(allRust, /"- RETRY IN \{seconds\}S"/, "The Oracle line's countdown copy moved");
assert.ok(brief.includes("RETRY IN NS"), "oracle-quiz.md must show the retry countdown copy");
const recallTicks = constant("ORACLE_RECALL_TICKS");
assert.ok(
  brief.includes(`every ${recallTicks} ticks`) && brief.includes("`ORACLE_RECALL_TICKS`"),
  "oracle-quiz.md must give the recall cadence",
);
assert.doesNotMatch(brief, /being implemented now/, "The brief still calls shipped work in progress");

/* ---------- The screen transcript is an existing mechanism ---------- */

if (/fn engine_transcript\(/.test(allRust)) {
  for (const [path, text] of [
    ["SKILL.md", skill],
    ["polish.md", polish],
    ["oracle-quiz.md", brief],
  ]) {
    assert.ok(text.includes("`engine_transcript`"), `${path} omits the screen transcript`);
  }
}

console.log("Docs contract: ok");
