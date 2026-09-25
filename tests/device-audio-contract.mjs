import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  DEFAULT_VOLUME,
  MAX_DRIFT_SECONDS,
  SCHEDULE_LEAD_SECONDS,
  VOLUME_LEVELS,
  VOLUME_STORAGE_KEY,
  createSpeaker,
  createTickClock,
  midiToHz,
  nextVolume,
  normalizeVolume,
  pulseHarmonics,
  stepVolume,
} from "../src/speaker.js";
import {
  WHEEL_DETENT_PX,
  WHEEL_GESTURE_GAP_MS,
  createWheelDetents,
} from "../src/device-shell.js";
import { reducedMotionCss } from "./contract-css.mjs";
import { bootShell } from "./shell-harness.mjs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const speakerSource = readFileSync(new URL("../src/speaker.js", import.meta.url), "utf8");
const rust = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));

function block(selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([\\s\\S]*?)\\}`));
  assert.ok(match, `Missing CSS block for ${selector}`);
  return match[1];
}

/* ---------- A recording stand-in for the WebAudio graph ---------- */

class FakeParam {
  constructor(value = 0) {
    this.value = value;
    this.events = [];
  }
  setValueAtTime(value, time) { this.events.push(["set", value, time]); }
  linearRampToValueAtTime(value, time) { this.events.push(["ramp", value, time]); }
  exponentialRampToValueAtTime(value, time) { this.events.push(["exp", value, time]); }
  setTargetAtTime(value, time) { this.events.push(["target", value, time]); }
  cancelScheduledValues(time) { this.events.push(["cancel", time]); }
}

class FakeNode {
  constructor(context, kind) {
    this.kind = kind;
    context.nodes.push(this);
  }
  connect(node) { this.output = node; return node; }
  disconnect() { this.output = null; }
}

class FakeSource extends FakeNode {
  start(time) { this.startAt = time; }
  stop(time) { this.stopAt = time; }
}

class FakeAudioContext {
  static created = 0;
  constructor() {
    FakeAudioContext.created += 1;
    this.state = "running";
    this.currentTime = 10;
    this.sampleRate = 8000;
    this.destination = { kind: "destination" };
    this.nodes = [];
    FakeAudioContext.last = this;
  }
  createGain() {
    const node = new FakeNode(this, "gain");
    node.gain = new FakeParam(1);
    return node;
  }
  createOscillator() {
    const node = new FakeSource(this, "oscillator");
    node.frequency = new FakeParam(440);
    node.setPeriodicWave = (wave) => { node.wave = wave; };
    return node;
  }
  createBufferSource() {
    const node = new FakeSource(this, "noise");
    node.playbackRate = new FakeParam(1);
    return node;
  }
  createBuffer(_channels, length) {
    const data = new Float32Array(length);
    return { getChannelData: () => data };
  }
  createPeriodicWave(real, imag) { return { real, imag }; }
  resume() { this.state = "running"; return Promise.resolve(); }
}

function memoryStorage(initial = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: (key) => (values.has(key) ? values.get(key) : null),
    setItem: (key, value) => values.set(key, String(value)),
    values,
  };
}

const note = (overrides) => ({
  voice: "pulse1",
  tick: 100,
  durationTicks: 6,
  pitch: 69,
  volume: 12,
  duty: 2,
  decay: 0,
  slide: 0,
  ...overrides,
});

/* ---------- Tick clock ---------- */

const clock = createTickClock();
assert.equal(clock.anchored, false);
assert.equal(clock.sync(600, 2), true, "The first batch anchors the clock");
assert.equal(clock.timeFor(600), 2 + SCHEDULE_LEAD_SECONDS, "Notes start slightly ahead of now");
assert.ok(SCHEDULE_LEAD_SECONDS >= 0.04 && SCHEDULE_LEAD_SECONDS <= 0.08, "The lookahead is roughly 60 ms");
assert.ok(Math.abs(clock.timeFor(660) - clock.timeFor(600) - 1) < 1e-9, "Sixty engine ticks are one second");
assert.equal(clock.sync(606, 2.1 + 0.02), false, "Ordinary polling jitter keeps the anchor");
assert.equal(MAX_DRIFT_SECONDS, 0.15, "Re-anchoring waits for 150 ms of drift");
assert.equal(clock.sync(612, 2.5), true, "A stalled poll re-anchors instead of bursting stale notes");
assert.equal(clock.timeFor(612), 2.5 + SCHEDULE_LEAD_SECONDS);

/* ---------- Volume wheel detents ---------- */

assert.deepEqual(VOLUME_LEVELS, ["mute", "low", "mid", "high"]);
assert.deepEqual(
  ["mute", "low", "mid", "high"].map(nextVolume),
  ["low", "mid", "high", "mute"],
  "V cycles MUTE, LOW, MID, HIGH and wraps",
);
assert.equal(stepVolume("high", 1), "high", "Rolling past the end stop stays at HIGH");
assert.equal(stepVolume("mute", -1), "mute", "Rolling below MUTE stays muted");
assert.equal(stepVolume("mid", -1), "low");
assert.equal(normalizeVolume("HIGH"), "high");
assert.equal(normalizeVolume("loud"), DEFAULT_VOLUME, "Unknown stored levels fall back safely");

/* ---------- Wheel travel, not wheel events, turns the detents ---------- */

const rollGesture = (roll, deltas, { start = 0, every = 8, mode = 0 } = {}) =>
  deltas.map((delta, index) => roll(delta, mode, start + index * every)).reduce((sum, turn) => sum + turn, 0);
assert.equal(WHEEL_DETENT_PX, 100, "One classic mouse notch is one detent");
assert.equal(WHEEL_GESTURE_GAP_MS, 200);
{
  const roll = createWheelDetents();
  assert.equal(rollGesture(roll, Array(30).fill(-4)), 1, "A light touchpad swipe turns one detent, not thirty");
  assert.equal(
    rollGesture(roll, Array(60).fill(-5), { start: 1000 }),
    3,
    "A long swipe turns one detent per 100 px of travel",
  );
  assert.equal(rollGesture(roll, Array(40).fill(3), { start: 2000 }), -1, "Rolling down turns the wheel down");
}
{
  const roll = createWheelDetents();
  const notches = [0, 60, 120, 180].map((at) => roll(-100, 0, at));
  assert.deepEqual(notches, [1, 1, 1, 1], "A notched mouse turns one detent per notch");
  assert.equal(roll(-120, 0, 1000), 1, "A single larger notch is still one detent");
  assert.equal(roll(-3, 1, 2000), 1, "Line-mode wheels turn one detent per notch");
  assert.equal(roll(-3, 1, 2050), 1);
  assert.equal(roll(1, 2, 3000), -1, "Page-mode wheels turn one detent per page");
  assert.equal(roll(0, 0, 3001), 0, "Horizontal-only events leave the wheel alone");
  assert.equal(roll(-2, 0, 3002), 1, "Reversing direction mid-gesture responds at once");
  assert.equal(roll(Number.NaN, 0, 3003), 0);
}
{
  // The real adapter wires the accumulator into the wheel it renders.
  const shell = await bootShell({ storage: { [VOLUME_STORAGE_KEY]: "low" } });
  const wheel = shell.el("volume-wheel");
  assert.equal(wheel.dataset.level, "low");
  for (let event = 0; event < 30; event += 1) {
    const wheeled = wheel.dispatch("wheel", { deltaY: -4 });
    assert.ok(wheeled.defaultPrevented, "The wheel never scrolls the page");
  }
  assert.equal(wheel.dataset.level, "mid", "Thirty small touchpad events turn the front wheel one detent");
  assert.equal(shell.storage.getItem(VOLUME_STORAGE_KEY), "mid");
  shell.close();
}

/* ---------- Pulse duty and pitch ---------- */

assert.equal(midiToHz(69), 440);
assert.ok(Math.abs(midiToHz(81) - 880) < 1e-9);
const half = pulseHarmonics(4, 8);
assert.ok(Math.abs(half.imag[1] - 2 / Math.PI) < 1e-6, "A 50% pulse is a square wave");
assert.ok(Math.abs(half.imag[2]) < 1e-6, "A square wave has no even harmonics");
const eighth = pulseHarmonics(1, 8);
assert.ok(Math.abs(eighth.imag[2]) > 0.01, "A 12.5% pulse keeps its even harmonics");

/* ---------- Gesture-gated speaker ---------- */

const storage = memoryStorage({ [VOLUME_STORAGE_KEY]: "low" });
const speaker = createSpeaker({ AudioContextClass: FakeAudioContext, storage });
assert.equal(speaker.volume, "low", "The wheel restores its persisted detent");
assert.equal(FakeAudioContext.created, 0, "No audio context exists before a user gesture");
assert.equal(speaker.play({ tick: 100, notes: [note()] }), 0, "Notes before a gesture are dropped");
assert.equal(FakeAudioContext.created, 0, "Engine audio alone never creates an audio context");

assert.equal(speaker.unlock(), true);
assert.equal(speaker.unlock(), true);
assert.equal(FakeAudioContext.created, 1, "Repeated gestures reuse one audio context");
const audio = FakeAudioContext.last;

assert.equal(
  speaker.play({
    tick: 100,
    notes: [
      note(),
      note({ voice: "wave", pitch: 50, duty: 4 }),
      note({ voice: "noise", pitch: 96, decay: 1 }),
      note({ voice: "pulse2", tick: 103, duty: 1, slide: 5 }),
      note({ voice: "unknown" }),
    ],
  }),
  4,
  "Each known voice schedules its note",
);
const sources = () => audio.nodes.filter((node) => node.kind === "oscillator" || node.kind === "noise");
assert.equal(sources().length, 4);
const [pulse, wave, noiseSource, slid] = sources();
assert.equal(pulse.startAt, 10 + SCHEDULE_LEAD_SECONDS, "The batch tick maps to now plus the lead");
assert.ok(Math.abs(slid.startAt - pulse.startAt - 3 / 60) < 1e-9, "Later ticks keep their spacing");
assert.ok(pulse.wave, "Pulse voices use a duty-shaped periodic wave");
assert.equal(wave.type, "triangle", "The wave voice is triangle-like");
assert.equal(noiseSource.loop, true, "The noise voice loops a noise buffer");
assert.ok(slid.frequency.events.some(([kind]) => kind === "exp"), "Slides glide the pitch");

// A zero-volume note cuts its voice on the requested tick.
speaker.play({ tick: 101, notes: [note({ tick: 101, volume: 0, durationTicks: 0 })] });
assert.ok(pulse.stopAt < pulse.startAt + 6 / 60, "A cut stops the sounding pulse early");
assert.equal(sources().length, 4, "A cut never starts a new source");

// Monophony: a new note on a sounding voice replaces it.
speaker.play({ tick: 102, notes: [note({ voice: "wave", tick: 102 })] });
assert.ok(wave.stopAt <= sources().at(-1).startAt + 0.01, "A voice sounds one note at a time");

/* ---------- Mute and persistence ---------- */

assert.equal(speaker.setVolume("mute"), "mute");
assert.equal(storage.values.get(VOLUME_STORAGE_KEY), "mute", "The detent persists");
const before = sources().length;
assert.equal(speaker.play({ tick: 200, notes: [note({ tick: 200 })] }), 0, "MUTE schedules nothing");
assert.equal(sources().length, before);
speaker.setVolume("high");
assert.equal(storage.values.get(VOLUME_STORAGE_KEY), "high");
assert.equal(speaker.play({ tick: 205, notes: [note({ tick: 205 })] }), 1);

const unavailable = createSpeaker({ AudioContextClass: null, storage: memoryStorage() });
assert.equal(unavailable.unlock(), false, "A missing WebAudio implementation stays silent");
assert.equal(unavailable.play({ tick: 1, notes: [note()] }), 0);
assert.equal(unavailable.volume, DEFAULT_VOLUME);

/* ---------- Presentation-only boundary ---------- */

assert.doesNotMatch(speakerSource, /invoke\(|__TAURI__/, "The speaker never talks to the engine itself");
assert.doesNotMatch(
  speakerSource,
  /\b(?:quiz|question|score|hearts?|wards?|oracle|datafall|cartridge|scene|screen)\b/i,
  "The speaker must not know game rules or state",
);
assert.match(adapter, /from "\.\/speaker\.js"/, "The adapter loads the speaker module");
assert.match(adapter, /let audioPending = false/, "Audio polling needs its own in-flight guard");
assert.match(
  adapter,
  /async function pollAudio\(\) \{\s*if \(audioPending\) return;[\s\S]*?invoke\("engine_audio"\)/,
  "The adapter polls engine_audio behind its guard",
);
assert.match(
  adapter,
  /async function drawFrame\(\) \{\s*void pollAudio\(\);/,
  "Every animation frame polls audio without awaiting it",
);
assert.match(
  adapter,
  /for \(const gesture of \["keydown", "pointerdown"\]\)[\s\S]*?speaker\.unlock\(\)/,
  "The first key press or device press wakes the speaker",
);
const keyV = adapter.match(/if \(event\.code === "KeyV"\) \{([^}]*)\}/);
assert.ok(keyV, "The device keydown handler needs a V branch");
assert.match(keyV[1], /if \(!event\.repeat\) setVolume\(nextVolume\(speaker\.volume\)\);/, "V cycles the volume wheel once per press");
assert.match(adapter, /command === "engine_audio"\) return \{ tick: 0, notes: \[\] \}/, "The browser demo is silent");
assert.match(rust, /fn engine_audio\(state: State<EngineState>\) -> audio::AudioBatch/, "Rust exposes engine_audio");
assert.match(rust, /generate_handler!\[[\s\S]*?engine_audio/, "engine_audio is registered with Tauri");

/* ---------- Volume hardware ---------- */

for (const id of ["volume-wheel", "rear-volume-wheel"]) {
  const wheel = html.match(new RegExp(`<div id="${id}"[^>]*>`))?.[0] || "";
  assert.match(wheel, /role="slider"/, `${id} must be a slider`);
  assert.match(wheel, /aria-valuemin="0"[^>]*aria-valuemax="3"/, `${id} needs four detents`);
  assert.match(wheel, /aria-valuetext="MID"/, `${id} needs a readable detent name`);
  assert.match(wheel, /tabindex="0"/, `${id} must be keyboard reachable`);
}
assert.match(html, /id="volume-meter"[\s\S]*?(?:class="volume-led"[\s\S]*?){3}/, "The meter needs three detent LEDs");
assert.match(block("#volume-wheel"), /right:\s*-12px/, "The front wheel protrudes from the right edge");
assert.match(block("#rear-volume-wheel"), /left:\s*-12px/, "The rear wheel mirrors onto the left edge");
for (const level of VOLUME_LEVELS) {
  assert.match(
    css,
    new RegExp(`\\.volume-wheel\\[data-level="${level}"\\] \\.volume-knurl \\{[^}]*translateY`),
    `The ${level} detent must visibly roll the knurl`,
  );
}
assert.match(css, /\.volume-meter\[data-level="mute"\]/, "MUTE needs its own visible state");
assert.match(
  reducedMotionCss(css),
  /\.volume-knurl\b[^{}]*\{[^}]*transition:\s*none/,
  "The wheel roll needs a reduced-motion state",
);
assert.match(adapter, /wheel\.setAttribute\("aria-valuetext", label\)/, "The wheel announces its detent");
assert.match(
  adapter,
  /wheel\.addEventListener\("pointerdown", \(event\) => \{\s*event\.preventDefault\(\);/,
  "Clicking the wheel must not steal focus from the D-pad",
);
assert.match(html, /<th>VOLUME<\/th>\s*<td>V<\/td>/, "The rear label lists the V volume key");
assert.match(packageJson.scripts.test, /node tests\/device-audio-contract\.mjs/, "npm test runs this contract");

console.log("Device audio contract OK: gesture-gated four-voice speaker, persisted volume wheel, engine-owned sound");
