import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");

for (const id of ["device-rotator", "device-front", "device-back", "rear-hotkeys"]) {
  assert.match(html, new RegExp(`id=["']${id}["']`), `Missing ${id} from the reversible shell`);
}

assert.match(html, /id="rear-cart-back"[^>]*>[\s\S]*?class="rear-cart-label"[\s\S]*?class="rear-cart-title"/, "The rear recess does not contain a complete cartridge face");
assert.match(css, /#rear-cart-back\.loaded\s*\{[^}]*height:\s*5\dpx/s, "The loaded rear cartridge is only a thin spine");
assert.match(adapter, /rearSlot\.querySelector\("\.rear-cart-title"\)\.textContent\s*=\s*cartridge\.title/, "The visible rear cartridge does not show the inserted cartridge title");

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

assert.match(css, /#shell-scale\s*\{[^}]*perspective:/s, "The shell wrapper has no 3D perspective");
assert.match(css, /#device-rotator\s*\{[^}]*transform-style:\s*preserve-3d/s, "The device does not preserve its two faces");
assert.match(css, /\.device-face\s*\{[^}]*backface-visibility:\s*hidden/s, "The shell faces can bleed through each other");
assert.match(css, /#device-back\s*\{[^}]*rotateY\(180deg\)/s, "The rear face is not mounted behind the front");
assert.match(css, /#shell-scale\.showing-back\s+#device-rotator\s*\{[^}]*rotateY\(180deg\)/s, "The shell has no visible rear state");
assert.match(css, /#device-back\s*\{[^}]*visibility:\s*hidden/s, "WebKit can expose the mirrored rear face before the turn");
assert.match(css, /#shell-scale\.showing-back\s+#device-back\s*\{[^}]*visibility:\s*visible/s, "The rear face never becomes explicitly visible");
assert.match(css, /#shell-scale\.showing-back\s+#device-front\s*\{[^}]*visibility:\s*hidden/s, "The front face remains visible behind the rear face");
assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)[\s\S]*?#device-rotator\s*\{[^}]*transition:\s*none/s, "The turn animation has no reduced-motion behavior");

assert.match(adapter, /function setShellBackVisible\(/, "The adapter does not own a shell-side state");
assert.match(adapter, /event\.code === "F1"/, "F1 does not turn the unit over");
assert.match(adapter, /ShiftLeft:\s*"select",\s*ShiftRight:\s*"select"/, "SELECT must remain mapped to Shift");
assert.match(adapter, /querySelectorAll\("\[data-device-turn\]"\)/, "Pointer turn controls are not wired");

console.log("Device reverse contract OK: reversible shell, complete hotkey label, SELECT preserved");
