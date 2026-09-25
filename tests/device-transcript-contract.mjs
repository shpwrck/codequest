import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const lib = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const engine = readFileSync(new URL("../src-tauri/src/engine.rs", import.meta.url), "utf8");
const transcript = readFileSync(new URL("../src-tauri/src/engine/transcript.rs", import.meta.url), "utf8");
const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));

function fn(name) {
  const start = adapter.search(new RegExp(`\\n  (?:async )?function ${name}\\(`));
  assert.ok(start >= 0, `Missing function ${name}`);
  const end = adapter.indexOf("\n  }\n", start);
  assert.ok(end > start, `Unterminated function ${name}`);
  return adapter.slice(start, end + 4);
}

/* ---------- A visually hidden, announced screen transcript ---------- */

const region = html.match(/<div id="engine-transcript"[^>]*><\/div>/)?.[0] || "";
assert.ok(region, "The screen needs a transcript region");
assert.match(region, /class="sr-only"/, "The transcript is hidden visually, not from assistive technology");
assert.match(region, /role="status"/);
assert.match(region, /aria-live="polite"/, "Screen changes must not interrupt the player");
assert.match(region, /aria-atomic="true"/, "Each screen is read as a whole");
assert.doesNotMatch(region, /\shidden\b|aria-hidden/, "The transcript must stay in the accessibility tree");
const screenInner = html.indexOf('id="screen-inner"');
assert.ok(
  html.indexOf('id="engine-canvas"') > screenInner
    && html.indexOf('id="engine-transcript"') > html.indexOf('id="engine-canvas"')
    && html.indexOf('id="engine-transcript"') < html.indexOf('id="device-boot"'),
  "The transcript belongs to the screen it describes",
);
const srOnly = css.match(/\.sr-only \{([\s\S]*?)\}/)?.[1] || "";
assert.match(srOnly, /position:\s*absolute/);
assert.match(srOnly, /clip:\s*rect\(0, 0, 0, 0\)/);
assert.doesNotMatch(srOnly, /display:\s*none|visibility:\s*hidden/, "display:none would silence the region");
assert.doesNotMatch(css, /#engine-transcript/, "No rule may restyle the region out of the accessibility tree");

/* ---------- The shell only presents what the engine publishes ---------- */

assert.match(adapter, /const screenTranscript = \$\("engine-transcript"\);/);
assert.match(adapter, /let transcriptPending = false;/, "Transcript polling needs its own in-flight guard");
assert.match(adapter, /let transcriptSeq = 0;/);
const cadence = Number(adapter.match(/const TRANSCRIPT_POLL_MS = (\d+);/)?.[1]);
assert.ok(cadence >= 100 && cadence <= 250, "The transcript is polled at a modest cadence");
assert.match(
  adapter,
  /async function drawFrame\(\) \{\s*void pollAudio\(\);\s*void pollTranscript\(\);/,
  "Every animation frame offers a transcript poll without awaiting it",
);
assert.equal(
  adapter.match(/screenTranscript\./g)?.length,
  1,
  "The shell writes the region in exactly one place and never composes its text",
);
assert.match(
  adapter,
  /command === "engine_transcript"\) return null;/,
  "The browser demo has no engine and so no transcript",
);

// Run the real polling function against a stand-in engine.
const poll = fn("pollTranscript");
function harness() {
  const calls = [];
  const replies = [];
  const clock = { now: 0 };
  const target = { textContent: "" };
  const invoke = (command, args) => {
    calls.push([command, args]);
    return replies.shift() ?? Promise.resolve(null);
  };
  const quiet = { error() {} };
  const build = new Function(
    "invoke",
    "performance",
    "screenTranscript",
    "TRANSCRIPT_POLL_MS",
    "console",
    `let transcriptPending = false;
     let transcriptPolledAt = -Infinity;
     let transcriptSeq = 0;
     ${poll}
     return pollTranscript;`,
  );
  return {
    calls,
    replies,
    clock,
    target,
    poll: build(invoke, { now: () => clock.now }, target, cadence, quiet),
  };
}

const live = harness();
live.replies.push(Promise.resolve({ seq: 1, text: "TRANSCRIPT TEST. Repository Oracle. Press Start." }));
await live.poll();
assert.deepEqual(live.calls, [["engine_transcript", { since: 0 }]], "The first poll asks for anything newer than nothing");
assert.equal(live.target.textContent, "TRANSCRIPT TEST. Repository Oracle. Press Start.");

live.clock.now = cadence / 2;
await live.poll();
assert.equal(live.calls.length, 1, "Animation frames between polls do not call the engine");

live.clock.now = cadence * 2;
live.target.textContent = "UNCHANGED";
await live.poll();
assert.deepEqual(live.calls[1], ["engine_transcript", { since: 1 }], "Later polls pass the last presented sequence");
assert.equal(live.target.textContent, "UNCHANGED", "No newer transcript leaves the region alone");

live.clock.now = cadence * 3;
live.replies.push(Promise.resolve({ seq: 1, text: "A REPEAT" }));
await live.poll();
assert.equal(live.target.textContent, "UNCHANGED", "A repeated sequence number is never re-announced");

live.clock.now = cadence * 4;
live.replies.push(Promise.resolve({ seq: 2, text: "" }));
await live.poll();
assert.equal(live.target.textContent, "", "Powering off clears the region");

live.clock.now = cadence * 5;
let release;
live.replies.push(new Promise((resolve) => { release = resolve; }));
const pending = live.poll();
live.clock.now = cadence * 7;
await live.poll();
assert.equal(live.calls.length, 5, "A slow reply blocks further polls until it lands");
release({ seq: 3, text: "Oracle Datafall." });
await pending;
assert.equal(live.target.textContent, "Oracle Datafall.");

const failing = harness();
failing.replies.push(Promise.reject(new Error("BEVY ENGINE STOPPED")));
await failing.poll();
failing.clock.now = cadence * 2;
await failing.poll();
assert.equal(failing.calls.length, 2, "A failed poll releases its guard");

/* ---------- The engine owns the words ---------- */

assert.match(
  lib,
  /fn engine_transcript\(state: State<EngineState>, since: u64\) -> Option<engine::TranscriptUpdate>/,
  "Rust exposes engine_transcript",
);
assert.match(lib, /generate_handler!\[[\s\S]*?engine_audio,\s*engine_transcript\s*\]/, "engine_transcript is registered beside engine_audio");
assert.match(engine, /pub fn transcript_since\(&self, seq: u64\) -> Option<TranscriptUpdate>/);
assert.match(engine, /let \(screen, sentences\) = engine\.transcript_sentences\(\);\s*if let Ok\(mut channel\) = shared_transcript\.lock\(\) \{\s*channel\.publish\(screen, sentences\);/);
assert.match(transcript, /pub\(super\) fn screen_transcript\(state: &GameState\) -> String/);
assert.match(transcript, /fn publish\(&mut self, screen: Screen, sentences: Vec<String>\) -> bool \{[\s\S]*?if latest == sentences\.as_slice\(\) \{\s*return false;/, "Only a changed transcript is published");
assert.match(transcript, /fn announcement\(old: &\[String\], new: &\[String\]\) -> String/, "A change on the same screen is announced on its own");
assert.match(packageJson.scripts.test, /node tests\/device-transcript-contract\.mjs/, "npm test runs this contract");

console.log("Device transcript contract OK: engine-written screen transcript in a polite live region, change-only polling");
