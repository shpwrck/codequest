/* CODE QUEST ADVANCE speaker.
 * The engine decides every sound and stamps each chip note with the engine
 * tick it starts on. This module is the handheld's speaker hardware: it maps
 * those ticks onto the WebAudio clock and plays four monophonic voices (two
 * pulses, a triangle wave, and noise) through the volume wheel. It holds no
 * game state and never chooses what to play. */

export const TICKS_PER_SECOND = 60;
/** Notes are scheduled this far ahead so polling jitter never clips them. */
export const SCHEDULE_LEAD_SECONDS = 0.06;
/** The tick clock re-anchors when it drifts further than this from real time. */
export const MAX_DRIFT_SECONDS = 0.15;
/** Notes later than this are dropped instead of played out of time. */
const MAX_LATENESS_SECONDS = 0.1;
const RELEASE_SECONDS = 0.004;

export const VOLUME_STORAGE_KEY = "cqa-volume";
export const VOLUME_LEVELS = Object.freeze(["mute", "low", "mid", "high"]);
export const VOLUME_LABELS = Object.freeze({ mute: "MUTE", low: "LOW", mid: "MID", high: "HIGH" });
export const DEFAULT_VOLUME = "mid";
const MASTER_GAIN = Object.freeze({ mute: 0, low: 0.2, mid: 0.45, high: 0.8 });
/* Per-voice ceilings keep four full-volume voices below clipping; the
 * triangle is naturally quieter than a pulse at the same amplitude. */
const VOICE_GAIN = Object.freeze({ pulse1: 0.2, pulse2: 0.2, wave: 0.38, noise: 0.14 });
const VOICES = Object.freeze(Object.keys(VOICE_GAIN));

export function normalizeVolume(value) {
  const level = String(value ?? "").trim().toLowerCase();
  return VOLUME_LEVELS.includes(level) ? level : DEFAULT_VOLUME;
}

/** The next detent when cycling with V or a click: MUTE, LOW, MID, HIGH, MUTE. */
export function nextVolume(level) {
  const index = VOLUME_LEVELS.indexOf(normalizeVolume(level));
  return VOLUME_LEVELS[(index + 1) % VOLUME_LEVELS.length];
}

/** Rolls the wheel one detent without wrapping past either end stop. */
export function stepVolume(level, direction) {
  const index = VOLUME_LEVELS.indexOf(normalizeVolume(level)) + Math.sign(direction);
  return VOLUME_LEVELS[Math.max(0, Math.min(VOLUME_LEVELS.length - 1, index))];
}

export function midiToHz(pitch) {
  return 440 * 2 ** ((pitch - 69) / 12);
}

/** Fourier series of a pulse wave whose high phase lasts `eighths` / 8. */
export function pulseHarmonics(eighths, count = 48) {
  const duty = Math.min(7, Math.max(1, Number(eighths) || 4)) / 8;
  const real = new Float32Array(count + 1);
  const imag = new Float32Array(count + 1);
  for (let harmonic = 1; harmonic <= count; harmonic += 1) {
    const phase = 2 * Math.PI * harmonic * duty;
    real[harmonic] = Math.sin(phase) / (Math.PI * harmonic);
    imag[harmonic] = (1 - Math.cos(phase)) / (Math.PI * harmonic);
  }
  return { real, imag };
}

/** Maps engine ticks onto an audio clock. The first batch anchors the map
 * slightly in the future; large drift (a stalled tab, a slow engine) anchors
 * it again instead of scheduling a burst of stale notes. */
export function createTickClock({
  lead = SCHEDULE_LEAD_SECONDS,
  maxDrift = MAX_DRIFT_SECONDS,
} = {}) {
  let anchorTick = null;
  let anchorTime = 0;
  const timeFor = (tick) => anchorTime + (tick - anchorTick) / TICKS_PER_SECOND;
  return {
    get anchored() {
      return anchorTick !== null;
    },
    /** Aligns the engine's current tick with `now`; true when it re-anchored. */
    sync(engineTick, now) {
      const target = now + lead;
      if (anchorTick !== null && Math.abs(timeFor(engineTick) - target) <= maxDrift) return false;
      anchorTick = engineTick;
      anchorTime = target;
      return true;
    },
    timeFor,
    reset() {
      anchorTick = null;
    },
  };
}

function browserStorage() {
  try {
    return globalThis.localStorage ?? null;
  } catch (_) {
    return null;
  }
}

function readStoredVolume(storage) {
  try {
    return normalizeVolume(storage?.getItem(VOLUME_STORAGE_KEY));
  } catch (_) {
    return DEFAULT_VOLUME;
  }
}

export function createSpeaker({
  AudioContextClass = globalThis.AudioContext || globalThis.webkitAudioContext,
  storage = browserStorage(),
} = {}) {
  let context = null;
  let master = null;
  let noiseBuffer = null;
  let volume = readStoredVolume(storage);
  const clock = createTickClock();
  const pulseWaves = new Map();
  const sounding = new Map();

  /** Creates or resumes the audio context. Call only from a user gesture. */
  function unlock() {
    if (!AudioContextClass) return false;
    if (!context) {
      try {
        context = new AudioContextClass();
      } catch (error) {
        console.warn("CQA: speaker unavailable", error);
        AudioContextClass = null;
        return false;
      }
      master = context.createGain();
      master.gain.value = MASTER_GAIN[volume];
      master.connect(context.destination);
    }
    if (context.state === "suspended") {
      context.resume?.()?.catch?.(() => {});
    }
    return true;
  }

  function pulseWave(eighths) {
    const key = Number(eighths) || 4;
    if (!pulseWaves.has(key)) {
      const { real, imag } = pulseHarmonics(key);
      pulseWaves.set(key, context.createPeriodicWave(real, imag));
    }
    return pulseWaves.get(key);
  }

  /* One second of the 15-bit linear-feedback noise used by chip sound. */
  function noise() {
    if (!noiseBuffer) {
      const length = Math.max(1, Math.round(context.sampleRate || 44100));
      noiseBuffer = context.createBuffer(1, length, context.sampleRate || 44100);
      const samples = noiseBuffer.getChannelData(0);
      let register = 0x7fff;
      for (let index = 0; index < length; index += 1) {
        if (index % 2 === 0) {
          const bit = (register ^ (register >> 1)) & 1;
          register = (register >> 1) | (bit << 14);
        }
        samples[index] = register & 1 ? 1 : -1;
      }
    }
    return noiseBuffer;
  }

  function stopVoice(voice, at) {
    const playing = sounding.get(voice);
    if (!playing) return;
    sounding.delete(voice);
    if (playing.end <= at) return;
    playing.gate.gain.cancelScheduledValues(at);
    playing.gate.gain.setValueAtTime(1, at);
    playing.gate.gain.linearRampToValueAtTime(0, at + RELEASE_SECONDS);
    try {
      playing.source.stop(at + RELEASE_SECONDS * 2);
    } catch (_) {
      /* Already stopped. */
    }
  }

  function createSource(voice, note, start, end) {
    const slide = Number(note.slide) || 0;
    if (voice === "noise") {
      const source = context.createBufferSource();
      source.buffer = noise();
      source.loop = true;
      const rate = (pitch) => Math.min(2, Math.max(0.0625, 2 ** ((pitch - 84) / 12)));
      source.playbackRate.setValueAtTime(rate(note.pitch), start);
      if (slide) source.playbackRate.exponentialRampToValueAtTime(rate(note.pitch + slide), end);
      return source;
    }
    const source = context.createOscillator();
    if (voice === "wave") source.type = "triangle";
    else source.setPeriodicWave(pulseWave(note.duty));
    source.frequency.setValueAtTime(midiToHz(note.pitch), start);
    if (slide) source.frequency.exponentialRampToValueAtTime(midiToHz(note.pitch + slide), end);
    return source;
  }

  function scheduleNote(note, start) {
    const voice = note.voice;
    stopVoice(voice, start);
    const volumeStep = Math.min(15, Math.max(0, Number(note.volume) || 0));
    const ticks = Math.max(0, Number(note.durationTicks) || 0);
    // A silent note is a cut: the voice simply stops.
    if (volumeStep === 0 || ticks === 0) return;

    const end = start + ticks / TICKS_PER_SECOND;
    const peak = (volumeStep / 15) * VOICE_GAIN[voice];
    const envelope = context.createGain();
    envelope.gain.setValueAtTime(peak, start);
    const decay = Math.max(0, Number(note.decay) || 0);
    if (decay > 0) {
      // The chip steps down one volume unit every `decay` ticks.
      const silentAt = start + (volumeStep * decay) / TICKS_PER_SECOND;
      if (silentAt <= end) envelope.gain.linearRampToValueAtTime(0, silentAt);
      else envelope.gain.linearRampToValueAtTime(peak * (1 - ticks / (volumeStep * decay)), end);
    }
    const gate = context.createGain();
    gate.gain.setValueAtTime(1, start);
    gate.gain.setValueAtTime(1, Math.max(start, end - RELEASE_SECONDS));
    gate.gain.linearRampToValueAtTime(0, end);

    const source = createSource(voice, note, start, end);
    source.connect(envelope);
    envelope.connect(gate);
    gate.connect(master);
    source.start(start);
    source.stop(end + RELEASE_SECONDS);
    source.onended = () => {
      if (sounding.get(voice)?.source === source) sounding.delete(voice);
      gate.disconnect();
    };
    sounding.set(voice, { source, gate, end });
  }

  function silence() {
    const now = context?.currentTime ?? 0;
    for (const voice of VOICES) stopVoice(voice, now);
    clock.reset();
  }

  /** Schedules one `engine_audio` batch; returns how many notes will sound. */
  function play(batch) {
    if (!context || !master || !Array.isArray(batch?.notes)) return 0;
    if (context.state !== "running" || volume === "mute") {
      // Drained notes are dropped rather than replayed as a burst later.
      silence();
      return 0;
    }
    const now = context.currentTime;
    clock.sync(Number(batch.tick) || 0, now);
    let scheduled = 0;
    for (const note of batch.notes) {
      if (!VOICES.includes(note?.voice)) continue;
      const due = clock.timeFor(Number(note.tick) || 0);
      if (due < now - MAX_LATENESS_SECONDS) continue;
      scheduleNote(note, Math.max(due, now));
      if (note.volume > 0 && note.durationTicks > 0) scheduled += 1;
    }
    return scheduled;
  }

  function setVolume(level) {
    volume = normalizeVolume(level);
    try {
      storage?.setItem(VOLUME_STORAGE_KEY, volume);
    } catch (_) {
      /* Private windows may refuse storage; the wheel still works. */
    }
    if (context && master) {
      master.gain.setTargetAtTime(MASTER_GAIN[volume], context.currentTime, 0.015);
      if (volume === "mute") silence();
    }
    return volume;
  }

  return {
    get volume() {
      return volume;
    },
    get unlocked() {
      return Boolean(context);
    },
    unlock,
    play,
    setVolume,
  };
}
