import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const engine = readFileSync(
  new URL("../src-tauri/src/engine.rs", import.meta.url),
  "utf8",
);
const font = readFileSync(
  new URL("../src-tauri/src/font5x7.rs", import.meta.url),
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
const glyphWidthPixels = numberFrom(font, /pub const GLYPH_WIDTH: i32 = (\d+);/, "glyph width");
const glyphHeightPixels = numberFrom(font, /pub const GLYPH_HEIGHT: i32 = (\d+);/, "glyph height");
const glyphAdvance = numberFrom(font, /pub const GLYPH_ADVANCE: i32 = (\d+);/, "glyph advance");

assert.equal(engineWidth, backingWidth, "Rust and canvas widths must match");
assert.equal(engineHeight, backingHeight, "Rust and canvas heights must match");
assert.equal(engineWidth, 240, "The game framebuffer must be 240 pixels wide");
assert.equal(engineHeight, 160, "The game framebuffer must be 160 pixels high");
assert.equal(
  cssWidth / backingWidth,
  cssHeight / backingHeight,
  "The framebuffer must use the same presentation scale on both axes",
);

const glyphWidth = glyphWidthPixels * (cssWidth / backingWidth);
const glyphHeight = glyphHeightPixels * (cssHeight / backingHeight);
assert.ok(
  glyphWidth >= 5 && glyphHeight >= 7,
  `Body glyphs render at ${glyphWidth}x${glyphHeight}px in the base LCD; expected at least 5x7px`,
);
assert.equal(Math.floor(backingWidth / glyphAdvance), 40, "The LCD must fit 40 text cells");

console.log(
  `Display contract OK: ${backingWidth}x${backingHeight} framebuffer, `
    + `${glyphWidth}x${glyphHeight}px base-LCD body glyphs`,
);
