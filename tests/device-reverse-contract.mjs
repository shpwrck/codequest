import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const rust = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");

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
for (const [property, label] of [["top", "top"], ["width", "width"]]) {
  assert.equal(
    pixels(rearCartridge, property, `rear cartridge ${label}`),
    pixels(frontCartridge, property, `front cartridge ${label}`),
    `Rear cartridge ${label} must match the front cartridge`,
  );
}
assert.ok(
  pixels(rearCartridge, "height", "rear cartridge height")
    > pixels(frontCartridge, "height", "front cartridge height"),
  "The rear view must reveal the cartridge body descending into the recess",
);
assert.match(rearCartridge, /linear-gradient\(#cdcedd,\s*#aaabbf\)/, "Rear cartridge is missing the front cartridge's gray surround");
assert.match(
  html,
  /id="rear-cart-back"[^>]*>[\s\S]*?class="rear-cart-thumb"[\s\S]*?class="rear-cart-sticker"/,
  "The selected cartridge treatment needs separate molded-tab and lower-sticker layers",
);
assert.doesNotMatch(css, /#rear-cart-back\.loaded::after\s*\{/, "Option C removes the upper color sticker");

const rearThumb = block("#rear-cart-back.loaded .rear-cart-thumb");
const rearSticker = block("#rear-cart-back.loaded .rear-cart-sticker");
const thumbTop = pixels(rearThumb, "top", "rear thumb tab top");
const thumbHeight = pixels(rearThumb, "height", "rear thumb tab height");
const stickerTop = pixels(rearSticker, "top", "rear lower sticker top");
assert.ok(thumbTop <= 5, "The molded thumb tab does not fill the removed top-sticker space");
assert.ok(thumbHeight >= 30, "The molded thumb tab is still the old narrow rib");
assert.ok(
  stickerTop - (thumbTop + thumbHeight) >= 5,
  "The lower sticker needs a deliberate gray gap beneath the thumb tab shadow",
);
assert.match(rearThumb, /linear-gradient\(#cdcedd,\s*#aaabbf\)/, "The thumb tab must use the cartridge's gray plastic");
assert.match(rearSticker, /var\(--cart-color/, "The lower sticker does not use the inserted cartridge color");
assert.ok(
  pixels(rearSticker, "left", "rear lower sticker left")
    > pixels(rearThumb, "left", "rear thumb tab left"),
  "The lower sticker must be narrower than the molded thumb tab",
);
assert.ok(
  pixels(rearSticker, "right", "rear lower sticker right")
    > pixels(rearThumb, "right", "rear thumb tab right"),
  "The lower sticker must be narrower than the molded thumb tab on both sides",
);
assert.ok(
  Number(block("#rear-cart-back").match(/z-index:\s*(\d+)/)?.[1]) >
    Number(block(".rear-cartridge-well").match(/z-index:\s*(\d+)/)?.[1]),
  "The loaded cartridge must sit inside and above the rear recess",
);
assert.match(html, /class="rear-cartridge-lip"/, "The rear recess needs a foreground lip to hold the cartridge");
assert.ok(
  Number(block(".rear-cartridge-lip").match(/z-index:\s*(\d+)/)?.[1]) >
    Number(block("#rear-cart-back").match(/z-index:\s*(\d+)/)?.[1]),
  "The recess lip must overlap the cartridge's lower edge",
);
const rearShellTop = pixels(block("#rear-shell"), "top", "rear shell top");
const lipTop = rearShellTop + pixels(block(".rear-cartridge-lip"), "top", "rear cartridge lip top");
const rearCartridgeTop = pixels(rearCartridge, "top", "rear cartridge top");
assert.ok(
  lipTop > rearCartridgeTop + stickerTop,
  "Part of the lower sticker must remain visible above the foreground lip",
);
assert.ok(
  lipTop < rearCartridgeTop + pixels(rearCartridge, "height", "rear cartridge height"),
  "The foreground lip must hide the cartridge's bottom edge",
);

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
]) {
  assert.match(
    html,
    new RegExp(`<th[^>]*>${control.replaceAll("/", "\\/")}<\\/th>\\s*<td[^>]*>${key.replaceAll("/", "\\/")}<\\/td>`),
    `Rear label is missing ${control} -> ${key}`,
  );
}

for (const shellOnlyControl of ["POWER", "SLOT", "TURN UNIT"]) {
  assert.doesNotMatch(html, new RegExp(`<th[^>]*>${shellOnlyControl}<\\/th>`), `${shellOnlyControl} should remain a pointer-only shell action`);
}

assert.match(html, /id="device-view-toggle"[^>]*role="switch"[^>]*aria-checked="false"/, "The shell needs a floating front/back switch");
assert.doesNotMatch(html, /data-device-turn/, "Molded shell badges must not act as highlighted turn controls");
assert.doesNotMatch(html, /<button class="shell-tag"/, "The front model badge must remain molded decoration");
assert.doesNotMatch(html, /<button class="rear-brand"/, "The rear brand mark must remain molded decoration");
assert.match(html, /id="device-back"[^>]*aria-hidden="true"/, "The rear face must start hidden from assistive technology");
assert.match(block("#device-view-toggle"), /position:\s*fixed/, "The front/back switch should float independently of the shell");
assert.match(css, /#device-view-toggle\.back-active[\s\S]*?\.view-toggle-knob/, "The slider does not expose its rear position");

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
assert.doesNotMatch(adapter, /event\.code === "(?:F1|KeyP|KeyC)"/, "Shell actions should not retain hidden hotkeys");
assert.match(adapter, /ShiftLeft:\s*"select",\s*ShiftRight:\s*"select"/, "SELECT must remain mapped to Shift");
assert.match(adapter, /viewToggle\.addEventListener\("click"/, "The floating front/back switch is not wired");
assert.match(adapter, /viewToggle\.setAttribute\("aria-checked",\s*String\(shellBackVisible\)\)/, "The slider's accessible state does not follow the visible face");

assert.match(html, /id="rear-serial">-------<\/div>/, "The empty serial plate needs a neutral revision placeholder");
assert.match(adapter, /rearSerial\.textContent\s*=\s*cartridge\?\.revision\s*\|\|\s*"-------"/, "The serial plate does not show the inserted repository revision");
assert.match(rust, /revision:\s*String/, "Cartridge metadata does not carry a revision");
assert.match(rust, /rev-parse",\s*"--short=7",\s*"HEAD"/, "Cartridge revisions are not loaded from short HEAD");

console.log("Device reverse contract OK: recessed cartridge, floating switch, compact hotkey label, revision serial");
