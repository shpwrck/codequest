import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");

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

console.log("Device pointer contract OK: contextual cartridge and power callouts are present");
