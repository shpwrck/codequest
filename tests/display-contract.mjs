import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const engine = readFileSync(
  new URL("../src-tauri/src/engine.rs", import.meta.url),
  "utf8",
);

function numberFrom(source, pattern, label) {
  const match = source.match(pattern);
  assert.ok(match, `Could not find ${label}`);
  return Number(match[1]);
}

const canvas = html.match(/<canvas\s+id="engine-canvas"[^>]*>/)?.[0];
assert.ok(canvas, "Could not find the engine canvas");
const cssBlock = css.match(/#engine-canvas\s*\{([\s\S]*?)\}/)?.[1];
assert.ok(cssBlock, "Could not find the engine canvas CSS");

const backingWidth = numberFrom(canvas, /width="(\d+)"/, "canvas backing width");
const backingHeight = numberFrom(canvas, /height="(\d+)"/, "canvas backing height");
const cssWidth = numberFrom(cssBlock, /width:\s*(\d+)px/, "canvas CSS width");
const cssHeight = numberFrom(cssBlock, /height:\s*(\d+)px/, "canvas CSS height");
const engineWidth = numberFrom(engine, /pub const WIDTH: usize = (\d+);/, "engine width");
const engineHeight = numberFrom(engine, /pub const HEIGHT: usize = (\d+);/, "engine height");
const bodyTextScale = numberFrom(
  engine,
  /const BODY_TEXT_SCALE: i32 = (\d+);/,
  "body text scale",
);

assert.equal(engineWidth, backingWidth, "Rust and canvas widths must match");
assert.equal(engineHeight, backingHeight, "Rust and canvas heights must match");
assert.equal(
  cssWidth / backingWidth,
  cssHeight / backingHeight,
  "The framebuffer must use the same presentation scale on both axes",
);

const glyphWidth = 8 * bodyTextScale * (cssWidth / backingWidth);
const glyphHeight = 8 * bodyTextScale * (cssHeight / backingHeight);
assert.ok(
  glyphWidth >= 8 && glyphHeight >= 8,
  `Body glyphs render at ${glyphWidth}x${glyphHeight}px in the base LCD; expected at least 8x8px`,
);

console.log(
  `Display contract OK: ${backingWidth}x${backingHeight} framebuffer, `
    + `${glyphWidth}x${glyphHeight}px base-LCD body glyphs`,
);
