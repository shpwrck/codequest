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

for (const control of ["cart", "power"]) {
  assert.match(
    html,
    new RegExp(`id=["']${control}-guide["']`),
    `Missing visible ${control} guide from the device shell`,
  );
  assert.match(
    css,
    new RegExp(`#${control}-guide\\s*\\{`),
    `Missing styling for the visible ${control} guide`,
  );
}

assert.match(css, /#cart-guide::before\s*\{[^}]*border-bottom-color:/s, "Cartridge guide has no arrow pointer");
assert.match(css, /#power-guide::after\s*\{[^}]*border-left-color:/s, "Power guide has no arrow pointer");
assert.match(adapter, /function updateControlGuides\(\)/, "Guide visibility is not synchronized with device state");
assert.match(adapter, /cartGuide\.classList\.toggle\("hidden", !needsCart\)/, "Cartridge guide never becomes visible");
assert.match(adapter, /powerGuide\.classList\.toggle\("hidden", !needsPower\)/, "Power guide never becomes visible");
assert.match(adapter, /powerGuide\.classList\.toggle\("switching-off", switchingOff\)/, "Power guide does not follow the switch position");
assert.match(
  adapter,
  /\$\("power-switch"\)\.addEventListener\("pointerdown", \(event\) => \{\s*event\.preventDefault\(\);/,
  "Mouse activation must not leave the power switch focused for the next keystroke",
);

const switchOffTop = pixels(block("#power-switch"), "top", "off switch top");
const switchOnTop = pixels(block("#power-switch.on"), "top", "on switch top");
const guideOffTop = pixels(block("#power-guide"), "top", "off-position guide top");
const guideOnTop = pixels(block("#power-guide.switching-off"), "top", "on-position guide top");
assert.ok(guideOffTop > guideOnTop, "TURN POWER ON pointer must align with the lowered off switch");
assert.equal(
  guideOffTop - guideOnTop,
  switchOffTop - switchOnTop,
  "Power pointer must move by the same distance as the physical switch",
);

console.log("Device pointer contract OK: contextual cartridge and power callouts are present");
