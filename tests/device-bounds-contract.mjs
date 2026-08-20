import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const tauri = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));

function block(selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([\\s\\S]*?)\\}`));
  assert.ok(match, `Missing CSS block for ${selector}`);
  return match[1];
}

function pixels(source, property, label) {
  const match = source.match(new RegExp(`${property}:\\s*(-?\\d+)(?:px)?`));
  assert.ok(match, `Missing ${label}`);
  return Number(match[1]);
}

function constant(name) {
  const match = adapter.match(new RegExp(`const ${name} = (\\d+);`));
  assert.ok(match, `Missing ${name}`);
  return Number(match[1]);
}

const deviceWidth = constant("DEVICE_WIDTH");
const deviceHeight = constant("DEVICE_HEIGHT");
const wrapper = block("#shell-scale");
const shell = block("#shell");
const shoulders = block("#shoulders");
const powerSwitch = block("#power-switch");

assert.equal(pixels(wrapper, "width", "wrapper width"), deviceWidth);
assert.equal(pixels(wrapper, "height", "wrapper height"), deviceHeight);
assert.equal(pixels(shoulders, "left", "shoulder left"), 0, "Shoulders must fit inside the wrapper");
assert.equal(pixels(shoulders, "top", "shoulder top"), 0, "Shoulders must fit inside the wrapper");
assert.ok(pixels(shell, "left", "shell left padding") >= 17, "Shell must reserve its left protrusion");
assert.ok(pixels(shell, "top", "shell top padding") >= 17, "Shell must reserve its top protrusion");
assert.ok(
  pixels(shell, "left", "shell left")
    + pixels(shell, "width", "shell width")
    - pixels(powerSwitch, "right", "power switch right")
    <= deviceWidth,
  "Power switch is clipped beyond the wrapper",
);
assert.ok(pixels(block("#cart-back.loaded"), "top", "loaded cartridge top") >= 0, "Loaded cartridge is clipped above the wrapper");
assert.ok(pixels(block("#cart-back.empty"), "top", "empty cartridge top") >= 0, "Empty cartridge is clipped above the wrapper");

const window = tauri.app.windows[0];
assert.ok(window.width >= deviceWidth * 2, "Default window must fit the complete device at 2x");
assert.ok(window.height >= deviceHeight * 2, "Default window must fit the complete device at 2x");

console.log(`Device bounds OK: ${deviceWidth}x${deviceHeight} complete-device fit box`);
