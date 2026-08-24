import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");

function block(selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([\\s\\S]*?)\\}`));
  assert.ok(match, `Missing CSS block for ${selector}`);
  return match[1];
}

function pixels(source, property, label) {
  const match = source.match(new RegExp(`${property}:\\s*(-?\\d+)px`));
  assert.ok(match, `Missing ${label}`);
  return Number(match[1]);
}

for (const id of ["device-rotator", "device-front", "device-back", "rear-hotkeys"]) {
  assert.match(html, new RegExp(`id=["']${id}["']`), `Missing ${id} from the reversible shell`);
}

assert.doesNotMatch(html, /rear-cart-(?:label|title)/, "The cartridge should not carry game text");
assert.doesNotMatch(adapter, /rearSlot\.querySelector/, "The rear cartridge should not render game text");

const frontCartridge = block("#cart-back.loaded");
const rearCartridge = block("#rear-cart-back.loaded");
for (const [property, label] of [["top", "top"], ["width", "width"], ["height", "height"]]) {
  assert.equal(
    pixels(rearCartridge, property, `rear cartridge ${label}`),
    pixels(frontCartridge, property, `front cartridge ${label}`),
    `Rear cartridge ${label} must match the front cartridge`,
  );
}
assert.match(rearCartridge, /linear-gradient\(#cdcedd,\s*#aaabbf\)/, "Rear cartridge is missing the front cartridge's gray surround");
assert.match(block("#rear-cart-back.loaded::after"), /var\(--cart-color/, "Rear cartridge does not use the inserted cartridge color");

const rearLabel = block("#rear-hotkeys");
const labelWidth = pixels(rearLabel, "width", "rear label width");
const labelHeight = pixels(rearLabel, "height", "rear label height");
assert.ok(labelWidth / labelHeight >= 4, `Rear label aspect ratio ${labelWidth / labelHeight} is too tall`);
assert.ok(labelWidth >= 300 && labelWidth <= 330, `Rear label width ${labelWidth}px does not match the reference proportion`);
assert.ok(labelHeight >= 70 && labelHeight <= 78, `Rear label height ${labelHeight}px does not match the reference proportion`);
assert.ok(pixels(block("#rear-hotkeys table"), "font-size", "rear label text size") >= 6, "Rear label text is too small to read");
assert.match(css, /#rear-hotkeys::after\s*\{/, "Rear label is missing the reference's center-bottom tab shape");

for (const [control, key] of [
  ["D-PAD", "ARROW KEYS"],
  ["A", "D"],
  ["B", "S"],
  ["START", "ENTER"],
  ["SELECT", "SHIFT"],
  ["L / R", "A / F"],
  ["POWER", "P"],
  ["SLOT", "C"],
  ["TURN UNIT", "F1"],
]) {
  assert.match(
    html,
    new RegExp(`<th[^>]*>${control.replaceAll("/", "\\/")}<\\/th>\\s*<td[^>]*>${key.replaceAll("/", "\\/")}<\\/td>`),
    `Rear label is missing ${control} -> ${key}`,
  );
}

assert.match(html, /data-device-turn/, "The shell has no pointer-accessible turn control");
assert.match(html, /id="device-back"[^>]*aria-hidden="true"/, "The rear face must start hidden from assistive technology");

assert.match(css, /#shell-scale\.turning\s+#device-rotator\s*\{[^}]*animation:\s*deviceTurn/s, "The whole device does not own the turn animation");
assert.match(css, /@keyframes deviceTurn\s*\{[\s\S]*?scaleX\(/, "The turn never carries the complete device edge-on");
assert.doesNotMatch(css, /rotateY\(/, "WebKit 3D rotation lets the transformed screen and bezel escape the shell face");
assert.match(css, /#shell-scale\.turning-to-back #device-front #bezel[\s\S]*?animation:\s*frontDisplayOut/s, "The front screen and bezel remain visible at the edge-on midpoint");
assert.match(css, /@keyframes frontDisplayOut\s*\{[\s\S]*?opacity:\s*0/s, "The front display assembly never clears before the face swap");
assert.match(css, /#device-back\s*\{[^}]*visibility:\s*hidden/s, "WebKit can expose the mirrored rear face before the turn");
assert.match(css, /#shell-scale\.showing-back\s+#device-back\s*\{[^}]*visibility:\s*visible/s, "The rear face never becomes explicitly visible");
assert.match(css, /#shell-scale\.showing-back\s+#device-front\s*\{[^}]*visibility:\s*hidden/s, "The front face remains visible behind the rear face");
assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)[\s\S]*?#shell-scale\.turning\s+#device-rotator\s*\{[^}]*animation:\s*none/s, "The turn animation has no reduced-motion behavior");

assert.ok(pixels(block(".rear-screw-sw"), "bottom", "lower-left screw inset") >= 40, "Lower-left screw overlaps the bottom shell shading");
assert.ok(pixels(block(".rear-screw-se"), "bottom", "lower-right screw inset") >= 40, "Lower-right screw overlaps the bottom shell shading");

assert.match(adapter, /function setShellBackVisible\(/, "The adapter does not own a shell-side state");
assert.match(adapter, /function turnShell\(/, "The adapter does not coordinate a whole-device turn");
assert.match(adapter, /TURN_DURATION_MS\s*\/\s*2/, "The shell face does not swap at the edge-on midpoint");
assert.match(adapter, /turning-to-back/, "The adapter does not distinguish the front-to-back display handoff");
assert.match(adapter, /matchMedia\("\(prefers-reduced-motion: reduce\)"\)/, "Reduced-motion users still receive the turn animation");
assert.match(adapter, /event\.code === "F1"/, "F1 does not turn the unit over");
assert.match(adapter, /ShiftLeft:\s*"select",\s*ShiftRight:\s*"select"/, "SELECT must remain mapped to Shift");
assert.match(adapter, /querySelectorAll\("\[data-device-turn\]"\)/, "Pointer turn controls are not wired");

console.log("Device reverse contract OK: reversible shell, complete hotkey label, SELECT preserved");
