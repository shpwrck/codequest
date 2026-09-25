/* CODE QUEST ADVANCE speaker.
 * The engine decides every sound and stamps each chip note with the engine
 * tick it starts on. This module is the handheld's speaker hardware: it maps
 * those ticks onto the WebAudio clock and plays four monophonic voices (two
 * pulses, a triangle wave, and noise) through the volume wheel. It holds no
 * game state and never chooses what to play. */

export const TICKS_PER_SECOND = 60;
/** Anchoring puts the engine's newest tick this far ahead of the audio clock,
 * so polling jitter never clips the notes that start on it. */
export const SCHEDULE_LEAD_SECONDS = 0.06;
/** The newest tick must land at least this far ahead. Closer means the engine
 * fell behind the audio clock (a slow frame), so the clock re-anchors rather
 * than start every later note late. */
export const MIN_LEAD_SECONDS = 0.02;
/** ...and at most this far ahead. Further means the audio clock fell behind
 * (or a late poll anchored late), so the clock re-anchors rather than keep
 * the extra delay on every later note. */
export const MAX_LEAD_SECONDS = 0.12;
/** Notes later than this do not start; they still end whatever their voice
 * was sounding, as they would have on time. */
export const MAX_LATENESS_SECONDS = 0.1;
const RELEASE_SECONDS = 0.004;
/* Noise brightness is the linear-feedback register's clock. At the reference
 * pitch it steps every second sample of the buffer; at the maximum rate it
 * steps every sample, the brightest a sampled register can be. */
const NOISE_REFERENCE_PITCH = 84;
const NOISE_MIN_RATE = 0.0625;
const NOISE_MAX_RATE = 2;
const NOISE_CEILING_PITCH = NOISE_REFERENCE_PITCH + 12 * Math.log2(NOISE_MAX_RATE);

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
 * `lead` seconds in the future. Every later batch keeps the anchor only while
 * the engine's newest tick still maps between `minLead` and `maxLead` ahead of
 * `now`; outside that window, in either direction, it anchors again. Engine
 * overruns (which only ever lose engine time) and audio-clock stalls therefore
 * cost one short jump instead of lag that builds up until every note is late. */
export function createTickClock({
  lead = SCHEDULE_LEAD_SECONDS,
  minLead = MIN_LEAD_SECONDS,
  maxLead = MAX_LEAD_SECONDS,
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
      if (anchorTick !== null) {
        const ahead = timeFor(engineTick) - now;
        if (ahead >= minLead && ahead <= maxLead) return false;
      }
      anchorTick = engineTick;
      anchorTime = now + lead;
      return true;
    },
    timeFor,
    reset() {
      anchorTick = null;
    },
  };
}

/** True unless the note is a cut: a zero volume step or zero length. */
function audible(note) {
  const volume = Math.min(15, Math.max(0, Number(note.volume) || 0));
  const ticks = Math.max(0, Number(note.durationTicks) || 0);
  return volume > 0 && ticks > 0;
}

/** Plans one `engine_audio` batch against `clock` at audio time `now`. Each
 * note on a known voice becomes `{ note, at, sounds }`: at `at` its voice stops
 * what it was sounding, and when `sounds` is true the note then starts there.
 * A cut never sounds. A note due more than MAX_LATENESS_SECONDS ago does not
 * start either, but it still ends its voice now: under monophony a late note
 * or cut means the voice's previous note is over, and dropping that would
 * leave a stale note ringing to its natural end. */
export function planBatch(clock, batch, now) {
  if (!Array.isArray(batch?.notes)) return [];
  clock.sync(Number(batch.tick) || 0, now);
  const plan = [];
  for (const note of batch.notes) {
    if (!VOICES.includes(note?.voice)) continue;
    const due = clock.timeFor(Number(note.tick) || 0);
    if (due < now - MAX_LATENESS_SECONDS) plan.push({ note, at: now, sounds: false });
    else plan.push({ note, at: Math.max(due, now), sounds: audible(note) });
  }
  return plan;
}

/** How the noise voice renders a brightness `pitch`: the register's playback
 * rate, and a highpass cutoff in Hz (0 for none). Rates stop at the brightest
 * a sampled register can be (a faster rate only skips samples of the same
 * white noise), so brightness past that ceiling thins the noise from below
 * instead: each octave past it removes half of the band that is left. Every
 * step of the brightness scale therefore sounds distinct. */
export function noiseColor(pitch, sampleRate = 44100) {
  const rate = 2 ** ((pitch - NOISE_REFERENCE_PITCH) / 12);
  const excess = Math.max(0, pitch - NOISE_CEILING_PITCH);
  return {
    rate: Math.min(NOISE_MAX_RATE, Math.max(NOISE_MIN_RATE, rate)),
    highpass: (sampleRate / 2) * (1 - 2 ** (-excess / 12)),
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

  /* The noise register, thinned by a highpass only when its brightness is
   * past the playback-rate ceiling at either end of a slide. */
  function createNoise(note, start, end) {
    const slide = Number(note.slide) || 0;
    const from = noiseColor(Number(note.pitch) || 0, context.sampleRate);
    const to = noiseColor((Number(note.pitch) || 0) + slide, context.sampleRate);
    const source = context.createBufferSource();
    source.buffer = noise();
    source.loop = true;
    source.playbackRate.setValueAtTime(from.rate, start);
    if (slide) source.playbackRate.exponentialRampToValueAtTime(to.rate, end);
    if (from.highpass <= 0 && to.highpass <= 0) return { source, output: source };
    const filter = context.createBiquadFilter();
    filter.type = "highpass";
    filter.frequency.setValueAtTime(from.highpass, start);
    if (slide) filter.frequency.linearRampToValueAtTime(to.highpass, end);
    source.connect(filter);
    return { source, output: filter };
  }

  function createSource(voice, note, start, end) {
    if (voice === "noise") return createNoise(note, start, end);
    const slide = Number(note.slide) || 0;
    const source = context.createOscillator();
    if (voice === "wave") source.type = "triangle";
    else source.setPeriodicWave(pulseWave(note.duty));
    source.frequency.setValueAtTime(midiToHz(note.pitch), start);
    if (slide) source.frequency.exponentialRampToValueAtTime(midiToHz(note.pitch + slide), end);
    return { source, output: source };
  }

  /* Starts an audible note, replacing whatever its voice was sounding. */
  function scheduleNote(note, start) {
    const voice = note.voice;
    stopVoice(voice, start);
    const volumeStep = Math.min(15, Math.max(0, Number(note.volume) || 0));
    const ticks = Math.max(0, Number(note.durationTicks) || 0);
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

    const { source, output } = createSource(voice, note, start, end);
    output.connect(envelope);
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
    let scheduled = 0;
    for (const { note, at, sounds } of planBatch(clock, batch, context.currentTime)) {
      if (!sounds) {
        stopVoice(note.voice, at);
        continue;
      }
      scheduleNote(note, at);
      scheduled += 1;
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
