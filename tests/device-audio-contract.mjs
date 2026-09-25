import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  DEFAULT_VOLUME,
  MAX_LATENESS_SECONDS,
  MAX_LEAD_SECONDS,
  MIN_LEAD_SECONDS,
  SCHEDULE_LEAD_SECONDS,
  VOLUME_LEVELS,
  VOLUME_STORAGE_KEY,
  createSpeaker,
  createTickClock,
  midiToHz,
  nextVolume,
  noiseColor,
  normalizeVolume,
  planBatch,
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
  createBiquadFilter() {
    const node = new FakeNode(this, "filter");
    node.type = "lowpass";
    node.frequency = new FakeParam(350);
    return node;
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
assert.equal(clock.sync(606, 2.1 - 0.02), false, "Jitter the other way keeps it too");
assert.ok(
  MIN_LEAD_SECONDS > 0 && MIN_LEAD_SECONDS < SCHEDULE_LEAD_SECONDS && SCHEDULE_LEAD_SECONDS < MAX_LEAD_SECONDS,
  "The anchor lead sits inside the window the clock tolerates",
);
assert.ok(MAX_LEAD_SECONDS <= 0.15, "The lookahead is bounded");
assert.equal(clock.sync(612, 2.5), true, "A stalled poll re-anchors instead of bursting stale notes");
assert.equal(clock.timeFor(612), 2.5 + SCHEDULE_LEAD_SECONDS);

// An engine overrun loses engine time, so the engine tick falls behind the
// audio clock. 100 ms behind, the newest tick would map 40 ms into the past
// and every note would start late at poll time; the clock re-anchors instead.
const behind = createTickClock();
behind.sync(600, 2);
assert.equal(behind.sync(606, 2.2), true, "An engine 100 ms behind re-anchors at once");
assert.equal(behind.timeFor(606), 2.2 + SCHEDULE_LEAD_SECONDS, "The newest tick gets the full lead back");
assert.equal(behind.sync(612, 2.3 + 0.03), false, "30 ms of lag inside the window keeps the anchor");
// An audio clock that fell behind leaves every later note scheduled too far
// ahead; the clock re-anchors instead of keeping that delay.
const ahead = createTickClock();
ahead.sync(600, 2);
assert.equal(ahead.sync(612, 2.1), true, "Notes mapping 160 ms ahead re-anchor");
assert.equal(ahead.timeFor(612), 2.1 + SCHEDULE_LEAD_SECONDS);

/* ---------- Scheduling a synthetic engine stream ---------- */

/** Drives planBatch with a 60 Hz engine and rAF-paced polls, both on one wall
 * clock. `stalls` are engine overruns: the engine resets its frame deadline
 * after one, so the stalled time is never caught up. `audioStalls` freeze
 * the audio clock, which then resumes behind wall time. Polls jitter by up to
 * 3 ms, arrive after 0.5-3 ms of IPC, and read an audio clock quantized to
 * 128-frame render quanta. Every engine tick emits one note. */
function simulateStream({ seconds = 20, stalls = [], audioStalls = [], seed = 7 } = {}) {
  let state = seed;
  const random = () => {
    state = (state * 1103515245 + 12345) % 2 ** 31;
    return state / 2 ** 31;
  };
  const engineTimes = [];
  const pendingStalls = [...stalls].sort((a, b) => a.at - b.at);
  for (let wall = 0; wall < seconds + 1; wall += 1 / 60) {
    while (pendingStalls.length && pendingStalls[0].at <= wall) wall += pendingStalls.shift().stall;
    engineTimes.push(wall);
  }
  const quantum = 128 / 48000;
  const audioTime = (wall) => {
    let lost = 0;
    for (const { at, stall } of audioStalls) lost += Math.min(stall, Math.max(0, wall - at));
    return Math.floor((10 + wall - lost) / quantum) * quantum;
  };
  const tickClock = createTickClock();
  const polls = [];
  let drained = -1;
  for (let frame = 1; frame / 60 < seconds; frame += 1) {
    const drainAt = frame / 60 + (random() - 0.5) * 0.006;
    let tick = drained;
    while (tick + 1 < engineTimes.length && engineTimes[tick + 1] <= drainAt) tick += 1;
    if (tick < 0) continue;
    const notes = [];
    for (let at = drained + 1; at <= tick; at += 1) {
      notes.push(note({ tick: at, durationTicks: 1, volume: 8 }));
    }
    drained = tick;
    const now = audioTime(drainAt + 0.0005 + random() * 0.0025);
    const offset = tickClock.anchored ? tickClock.timeFor(0) : null;
    const plan = planBatch(tickClock, { tick, notes }, now);
    polls.push({
      wall: drainAt,
      now,
      tick,
      notes,
      plan,
      lead: tickClock.timeFor(tick) - now,
      reanchored: offset === null || Math.abs(tickClock.timeFor(0) - offset) > 1e-9,
    });
  }
  return polls;
}

function assertOnTime(polls, label) {
  for (const poll of polls) {
    assert.ok(
      poll.lead >= MIN_LEAD_SECONDS - 1e-9 && poll.lead <= MAX_LEAD_SECONDS + 1e-9,
      `${label}: the newest tick maps ${poll.lead.toFixed(3)} s ahead at audio time ${poll.now.toFixed(3)}`,
    );
    // A poll after an ordinary frame carries a tick or two; none may start
    // late at poll time or be dropped.
    if (poll.notes.length > 2) continue;
    for (const step of poll.plan) {
      assert.ok(step.sounds && step.at > poll.now, `${label}: tick ${step.note.tick} started late`);
    }
  }
}

const steady = simulateStream();
assert.equal(steady.filter((poll) => poll.reanchored).length, 1, "Polling jitter alone never re-anchors");
assertOnTime(steady, "steady");
const starts = steady.flatMap((poll) => poll.plan.map((step) => step.at));
assert.equal(starts.length, steady.at(-1).tick + 1, "Every engine tick's note is scheduled");
assert.ok(
  starts.every((at, index) => index === 0 || Math.abs(at - starts[index - 1] - 1 / 60) < 1e-9),
  "Note starts follow engine ticks exactly, not poll timing",
);

// Hitches: two long overruns, then a run of small ones that add up to 100 ms.
const stalls = [
  { at: 3, stall: 0.1 },
  { at: 7, stall: 0.07 },
  ...Array.from({ length: 20 }, (_, index) => ({ at: 11 + index * 0.2, stall: 0.005 })),
];
/** Re-anchors after the first happen only while, or just after, the clock
 * was disturbed; never from jitter alone. */
function assertReanchorsFollow(polls, events, label) {
  const reanchors = polls.filter((poll) => poll.reanchored).slice(1);
  for (const poll of reanchors) {
    assert.ok(
      events.some(({ at, stall }) => poll.wall >= at && poll.wall < at + stall + 0.25),
      `${label}: re-anchored at ${poll.wall.toFixed(3)} s with nothing disturbing the clock`,
    );
  }
  return reanchors.length;
}

const hitched = simulateStream({ stalls });
assertOnTime(hitched, "engine overruns");
const overrunReanchors = assertReanchorsFollow(hitched, stalls, "engine overruns");
assert.ok(overrunReanchors >= 3, "Long overruns and accumulated small ones each re-anchor");
assert.ok(overrunReanchors < stalls.length / 2, "Small overruns re-anchor only once enough lag builds up");
const lastBeat = hitched.slice(-60).flatMap((poll) => poll.plan.map((step) => step.at));
assert.ok(
  lastBeat.every((at, index) => index === 0 || Math.abs(at - lastBeat[index - 1] - 1 / 60) < 1e-9),
  "After overruns the rhythm follows engine ticks again",
);

const audioStalls = [{ at: 5, stall: 0.1 }, { at: 9, stall: 0.2 }];
const audioBehind = simulateStream({ audioStalls });
assertOnTime(audioBehind, "audio clock stalls");
assert.ok(
  assertReanchorsFollow(audioBehind, audioStalls, "audio clock stalls") >= audioStalls.length,
  "An audio clock that falls behind re-anchors the lookahead back down",
);

// After a long poll gap the backlog is old: notes past the lateness bound do
// not start, but each still ends its voice at once.
const backlog = createTickClock();
backlog.sync(100, 5);
const gap = planBatch(
  backlog,
  {
    tick: 130,
    notes: [
      note({ tick: 110 }),
      note({ voice: "wave", tick: 112, volume: 0, durationTicks: 0 }),
      note({ tick: 129 }),
      note({ voice: "mystery", tick: 129 }),
    ],
  },
  5.5,
);
assert.equal(gap.length, 3, "Unknown voices are skipped");
assert.deepEqual(
  gap.map(({ note: { voice, tick }, at, sounds }) => [voice, tick, Number(at.toFixed(4)), sounds]),
  [
    ["pulse1", 110, 5.5, false],
    ["wave", 112, 5.5, false],
    ["pulse1", 129, Number((5.5 + SCHEDULE_LEAD_SECONDS - 1 / 60).toFixed(4)), true],
  ],
  "Late notes and cuts end their voice now; notes in time keep their tick",
);
assert.ok(MAX_LATENESS_SECONDS >= 0.05, "Only clearly stale notes are skipped");

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

// A cut that arrives too late to land on its tick still ends the voice now,
// instead of leaving a long loop note ringing to its natural end.
const lateSpeaker = createSpeaker({ AudioContextClass: FakeAudioContext, storage: memoryStorage() });
lateSpeaker.unlock();
const lateAudio = FakeAudioContext.last;
const lateSources = () => lateAudio.nodes.filter((node) => node.kind === "oscillator" || node.kind === "noise");
lateAudio.currentTime = 20;
lateSpeaker.play({
  tick: 100,
  notes: [note({ voice: "wave", tick: 100, durationTicks: 90 }), note({ tick: 100, durationTicks: 90 })],
});
const [longWave, longPulse] = lateSources();
assert.ok(longWave.stopAt > 21.5, "The loop note would ring for a second and a half");
lateAudio.currentTime = 20.36;
assert.equal(
  lateSpeaker.play({
    tick: 118,
    notes: [
      note({ voice: "wave", tick: 101, volume: 0, durationTicks: 0 }),
      note({ tick: 102, durationTicks: 90 }),
    ],
  }),
  0,
  "Stale notes do not start",
);
assert.equal(lateSources().length, 2, "A late note never starts a source");
assert.ok(Math.abs(longWave.stopAt - 20.36) < 0.01, "A late cut silences its voice immediately");
assert.ok(Math.abs(longPulse.stopAt - 20.36) < 0.01, "A late replacement ends the note it replaces");
assert.ok(
  longWave.output.output.gain.events.some(([kind, time]) => kind === "cancel" && time === 20.36),
  "The silenced voice's gate is released now",
);

// A clock that jumps back maps new ticks before notes already planned. Those
// notes must never sound after the later ticks, nor on top of them. Each test
// note's pitch is its tick, so a source names the tick it plays.
const RELEASE_TOLERANCE = 0.008 + 1e-9;
function soundingNotes(context) {
  return context.nodes
    .filter((node) => node.kind === "oscillator")
    .map((source) => ({
      source,
      tick: Math.round(69 + 12 * Math.log2(source.frequency.events[0][1] / 440)),
      start: source.startAt,
      stop: source.stopAt,
      connected: source.output.output.output != null,
    }))
    .filter(({ start, stop, connected }) => stop > start && connected);
}
function assertTickOrderAndMonophony(context, voiceOf, label) {
  const heard = soundingNotes(context).map((entry) => ({ ...entry, voice: voiceOf(entry.tick) }));
  for (const earlier of heard) {
    for (const later of heard) {
      if (earlier.tick >= later.tick) continue;
      assert.ok(
        earlier.start <= later.start + 1e-9,
        `${label}: tick ${earlier.tick} starts at ${earlier.start.toFixed(3)}, after tick ${later.tick} at ${later.start.toFixed(3)}`,
      );
      if (earlier.voice === later.voice) {
        assert.ok(
          earlier.stop <= later.start + RELEASE_TOLERANCE,
          `${label}: ${earlier.voice} plays tick ${earlier.tick} over tick ${later.tick}`,
        );
      }
    }
  }
  return heard;
}
{
  // The review repro: the audio clock freezes while the engine keeps ticking,
  // until tick 104 maps past the lookahead and the clock re-anchors back.
  const frozen = createSpeaker({ AudioContextClass: FakeAudioContext, storage: memoryStorage() });
  frozen.unlock();
  const frozenAudio = FakeAudioContext.last;
  for (let tick = 100; tick <= 104; tick += 1) {
    frozen.play({ tick, notes: [note({ tick, pitch: tick, durationTicks: 30 })] });
  }
  const heard = assertTickOrderAndMonophony(frozenAudio, () => "pulse1", "frozen audio clock");
  assert.deepEqual(heard.map(({ tick }) => tick), [104], "Material the jump overtook never starts; the newest tick plays");
  assert.equal(heard[0].start, 10 + SCHEDULE_LEAD_SECONDS);
}
{
  // Across voices too: a later tick on one voice never plays before an
  // earlier tick planned on another.
  const voices = ["pulse1", "pulse2", "wave"];
  const voiceOf = (tick) => voices[tick % voices.length];
  const frozen = createSpeaker({ AudioContextClass: FakeAudioContext, storage: memoryStorage() });
  frozen.unlock();
  const frozenAudio = FakeAudioContext.last;
  for (let tick = 100; tick <= 112; tick += 1) {
    frozen.play({ tick, notes: [note({ voice: voiceOf(tick), tick, pitch: tick, durationTicks: 30 })] });
  }
  const heard = assertTickOrderAndMonophony(frozenAudio, voiceOf, "frozen clock, three voices");
  assert.ok(heard.some(({ tick }) => tick === 112), "The newest tick still plays");
}
{
  // The synthetic stream with audio-clock stalls, through the real speaker.
  const streamed = createSpeaker({ AudioContextClass: FakeAudioContext, storage: memoryStorage() });
  streamed.unlock();
  const streamAudio = FakeAudioContext.last;
  const polls = simulateStream({ seconds: 11, audioStalls: [{ at: 5, stall: 0.1 }, { at: 9, stall: 0.2 }] });
  for (const poll of polls) {
    streamAudio.currentTime = poll.now;
    streamed.play({
      tick: poll.tick,
      notes: poll.notes.map((entry) => ({ ...entry, pitch: entry.tick, durationTicks: 3 })),
    });
  }
  const heard = assertTickOrderAndMonophony(streamAudio, () => "pulse1", "audio clock stalls");
  assert.ok(heard.length > polls.at(-1).tick * 0.9, "Only the few notes a jump overtakes are dropped");
}

// Noise brightness: every authored step sounds distinct, even past the
// playback-rate ceiling (the archival tick is brighter than the hat).
const NOISE_SCALE = [48, 72, 84, 96, 100];
for (const pitch of NOISE_SCALE.filter((value) => value <= 96)) {
  assert.deepEqual(
    noiseColor(pitch, 48000),
    { rate: 2 ** ((pitch - 84) / 12), highpass: 0 },
    `Noise ${pitch} keeps its register rate and no filter`,
  );
}
for (let index = 1; index < NOISE_SCALE.length; index += 1) {
  const darker = noiseColor(NOISE_SCALE[index - 1], 48000);
  const brighter = noiseColor(NOISE_SCALE[index], 48000);
  assert.ok(
    brighter.rate > darker.rate || brighter.highpass > darker.highpass,
    `Noise ${NOISE_SCALE[index]} must sound brighter than ${NOISE_SCALE[index - 1]}`,
  );
}
const tickColor = noiseColor(100, 48000);
assert.equal(tickColor.rate, 2, "The register never runs past one step per sample");
assert.ok(tickColor.highpass > 2000 && tickColor.highpass < 12000, "The tick is thinned to a dry, bright click");
assert.ok(noiseColor(160, 48000).highpass < 24000, "The highpass stays below Nyquist");
lateAudio.currentTime = 30;
lateSpeaker.play({
  tick: 300,
  notes: [note({ voice: "noise", tick: 300, pitch: 96 }), note({ voice: "noise", tick: 306, pitch: 100 })],
});
const [hatNoise, tickNoise] = lateSources().slice(-2);
assert.equal(hatNoise.output.kind, "gain", "The hat plays the register unfiltered");
assert.equal(tickNoise.output.kind, "filter", "The tick passes through a filter");
assert.equal(tickNoise.output.type, "highpass");
assert.equal(tickNoise.output.frequency.events[0][1], noiseColor(100, lateAudio.sampleRate).highpass);
assert.deepEqual(
  [hatNoise, tickNoise].map((source) => source.playbackRate.events[0][1]),
  [2, 2],
  "Both run at the ceiling rate; only the filter tells them apart",
);

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
