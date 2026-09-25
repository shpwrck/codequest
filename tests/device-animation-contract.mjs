import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const rust = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");

function block(selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([\\s\\S]*?)\\}`));
  assert.ok(match, `Missing CSS block for ${selector}`);
  return match[1];
}

const guide = block(".control-guide");
const cartridgeGlow = block("#cart-back.guided");
const powerGlow = block("#power-switch.guided");
const batteryTabGlow = block(".rear-battery-door.guided .rear-latch");
const batteryBayGlow = block(".battery-bay.guided");
const canvas = block("#engine-canvas");

for (const [name, animation] of [
  ["guide movement", guide],
  ["cartridge glow", cartridgeGlow],
  ["power glow", powerGlow],
  ["battery-tab glow", batteryTabGlow],
  ["battery-bay glow", batteryBayGlow],
]) {
  assert.doesNotMatch(animation, /steps\(/, `${name} uses visibly stepped timing`);
  assert.match(animation, /ease-in-out/, `${name} does not use continuous ease-in-out timing`);
}

assert.match(css, /@keyframes guideBob\s*\{[^}]*transform:/s, "Guide movement is not compositor-based");
assert.doesNotMatch(css, /@keyframes guideBob\s*\{[^}]*margin-top:/s, "Guide movement triggers layout every frame");
assert.match(canvas, /image-rendering:\s*pixelated/, "Smoothing the shell must not blur the game framebuffer");

// The framebuffer's own decorative motion follows the host preference too: the
// shell forwards prefers-reduced-motion (initially and on change) and the
// engine, not the shell, decides what freezes.
const syncMotion = adapter.match(/function syncReducedMotion\(\) \{([\s\S]*?)\n  \}/);
assert.ok(syncMotion, "The shell must forward the reduced-motion preference");
assert.match(syncMotion[1], /prefers-reduced-motion: reduce/);
assert.match(syncMotion[1], /invoke\("engine_set_reduced_motion", \{ reduced \}\)/);
assert.match(adapter, /addEventListener\?\.\("change", syncReducedMotion\)/, "Preference changes must reach the engine");
assert.match(rust, /fn engine_set_reduced_motion\(/);
assert.match(rust, /engine_set_reduced_motion,/, "The reduced-motion command must be registered");

console.log("Device animation contract OK: smooth shell motion, crisp framebuffer, forwarded reduced motion");
