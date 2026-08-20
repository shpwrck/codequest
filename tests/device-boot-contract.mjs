import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");

assert.match(html, /id="device-boot"/, "The fixed LCD boot overlay is missing");
assert.match(html, /class="boot-word"[^>]*>CODEQUEST</, "The legacy CODEQUEST mark is missing");
assert.match(html, /class="boot-maker"[^>]*>\s*<span>CQA SYSTEM<\/span>/, "The maker badge is missing");

const bootCss = css.match(/#device-boot\s*\{([\s\S]*?)\}/)?.[1];
assert.ok(bootCss, "The boot overlay CSS is missing");
assert.match(bootCss, /position:\s*absolute/, "The boot overlay must not affect LCD layout");
assert.match(bootCss, /inset:\s*0/, "The boot overlay must cover the existing LCD exactly");
assert.match(css, /@keyframes bootDrop/, "The original drop animation is missing");
assert.match(css, /@keyframes bootShine/, "The original shine animation is missing");
assert.match(css, /@keyframes bootSub/, "The original maker animation is missing");

assert.match(adapter, /invoke\("engine_finish_boot"\)/, "Device boot must explicitly release Bevy");

console.log("Device boot contract OK: fixed overlay with drop, shine, and engine handoff");
