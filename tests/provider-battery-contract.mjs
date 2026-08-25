import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const rust = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const engine = readFileSync(new URL("../src-tauri/src/engine.rs", import.meta.url), "utf8");
const externalTools = readFileSync(
  new URL("../src-tauri/src/external_tools.rs", import.meta.url),
  "utf8",
);

function block(selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([\\s\\S]*?)\\}`));
  assert.ok(match, `Missing CSS block for ${selector}`);
  return match[1];
}

function pixels(selector, property) {
  const match = block(selector).match(new RegExp(`${property}:\\s*(\\d+)px`));
  assert.ok(match, `Missing ${property} on ${selector}`);
  return Number(match[1]);
}

function backgroundColors(selector) {
  const match = block(selector).match(/background:\s*([^;]+);/);
  assert.ok(match, `Missing background on ${selector}`);
  return [...new Set([...match[1].matchAll(/#[0-9a-f]{6}/gi)].map(([color]) => color.toLowerCase()))];
}

for (const id of [
  "battery-compartment",
  "battery-bay",
  "battery-lid-slot",
  "battery-pack",
  "battery-chooser",
  "battery-status",
  "battery-door",
]) {
  assert.match(html, new RegExp(`id=["']${id}["']`), `Missing ${id}`);
}

assert.match(
  html,
  /id="battery-door"[^>]*aria-expanded="false"/,
  "The battery door must expose its closed state accessibly",
);
assert.match(html, /data-provider="codex"/, "The compartment is missing Codex batteries");
assert.match(html, /data-provider="claude"/, "The compartment is missing Claude batteries");
assert.equal(
  [...html.matchAll(/class="aa-battery /g)].length,
  2,
  "The installed provider must be represented by exactly two AA cells",
);
assert.equal(
  [...html.matchAll(/class="battery-contact /g)].length,
  4,
  "The two AA bays must expose all four electrical contacts",
);
assert.equal(
  [...html.matchAll(/<svg class="battery-contact spring/g)].length,
  2,
  "Each negative AA terminal must use a drawn spring",
);
assert.equal(
  [...html.matchAll(/class="spring-coil"/g)].length,
  2,
  "Each negative lead must contain a conical coil",
);
assert.match(html, /battery-contact spring top-left/, "The top AA spring must match the reference's negative end");
assert.match(html, /battery-contact leaf top-right/, "The top AA positive lead must match the reference");
assert.match(html, /battery-contact leaf bottom-left/, "The lower AA positive lead must match the reference");
assert.match(html, /battery-contact spring bottom-right/, "The lower AA spring must match the reference's negative end");
assert.equal(
  [...html.matchAll(/class="battery-logo|class="codex-logo-mark/g)].length,
  0,
  "Provider cells must not include a Codex logo or its surrounding arc",
);
assert.equal(
  [...html.matchAll(/class="battery-cradle /g)].length,
  2,
  "Each AA cell must sit in a visible molded cradle",
);
assert.match(
  html,
  /<button id="battery-door"[\s\S]*?<span class="rear-latch"[\s\S]*?<\/button>/,
  "The light latch must be part of the removable battery cover",
);
assert.match(
  html,
  /id="battery-bay"[\s\S]*?<button id="battery-lid-slot"[^>]*aria-label="Replace battery cover"/,
  "The open bay must retain the centered receptacle used to replace the cover",
);

assert.match(block(".battery-compartment"), /position:\s*absolute/, "The battery bay must belong to the rear shell");
assert.match(
  block(".battery-compartment.open"),
  /overflow:\s*visible/,
  "The removed cover must remain fully visible below the device",
);
assert.match(
  block("#rear-shell"),
  /overflow:\s*visible/,
  "The rear shell must not clip the fully removed cover",
);
assert.doesNotMatch(css, /#shell-scale\.battery-door-open/, "Opening the cover must not resize the device");
const openDoor = block(".battery-compartment.open .rear-battery-door");
assert.match(
  openDoor,
  /transform:\s*translateY\(/,
  "Opening the compartment must move the physical door",
);
const coverTravel = Number(openDoor.match(/translateY\((\d+)px\)/)?.[1] || 0);
assert.ok(coverTravel >= 170, "The removed cover must continue completely off screen");
assert.doesNotMatch(
  openDoor,
  /rotate\(/,
  "The battery cover must lift away without spinning",
);
assert.doesNotMatch(
  css,
  /\.battery-compartment\.open\s+\.rear-latch\s*\{[^}]*rotate\(/s,
  "The latch must not rotate independently from the cover",
);
const batteryBody = block(".aa-battery");
assert.match(batteryBody, /linear-gradient\(to bottom/, "AA cells need cylindrical cross-body shading");
assert.match(batteryBody, /border-radius:\s*4px/, "AA cells need the reference's squared side-profile barrel");
const positiveNub = css.match(
  /\.aa-battery\.top::after,\s*\.aa-battery\.bottom::before\s*\{([\s\S]*?)\}/,
)?.[1] || "";
assert.match(positiveNub, /width:\s*11px/, "Positive AA terminals need a distinct metal nub");
assert.match(positiveNub, /height:\s*21px/, "Positive AA terminal nubs must remain compact");
assert.match(positiveNub, /border-radius:\s*3px/, "Positive AA terminal nubs need softly rounded square edges");
assert.doesNotMatch(css, /\.aa-battery\.(?:top::after|bottom::before)\s*\{[^}]*50%/s, "Positive AA terminal nubs must not be semicircular caps");
assert.match(
  css,
  /(?:^|\n)\.aa-battery\.top::after\s*\{\s*left:\s*calc\(100% \+ 1px\)/,
  "The top battery's positive nub must begin outside the barrel edge",
);
assert.match(
  css,
  /(?:^|\n)\.aa-battery\.bottom::before\s*\{\s*right:\s*calc\(100% \+ 1px\)/,
  "The reversed battery's positive nub must begin outside the barrel edge",
);
const negativeEnd = css.match(
  /\.aa-battery\.top::before,\s*\.aa-battery\.bottom::after\s*\{([\s\S]*?)\}/,
)?.[1] || "";
assert.match(negativeEnd, /display:\s*none/, "Negative AA ends must not add a silver box over the spring");
assert.match(block(".battery-contact.spring"), /overflow:\s*visible/, "The conical springs must remain fully visible");
assert.match(css, /\.spring-coil\s*\{[^}]*stroke:/s, "The conical spring wire must be visibly drawn");
assert.match(block(".battery-contact.leaf"), /background:/, "The flat electrical leads must be visible");
assert.ok(pixels(".battery-pack", "left") <= 30, "Reference AA cells must reach the compact end contacts");
assert.ok(pixels(".battery-pack", "width") >= 260, "Reference AA cells must span the molded compartment");
assert.equal(pixels(".battery-pack", "top"), 14, "The cells must leave room for the cover's centered tab opening");
assert.equal(pixels(".battery-contact.top-left", "top"), 21, "The upper spring must align with the AA barrel");
assert.equal(pixels(".battery-contact.top-right", "top"), 23, "The upper positive lead must align with its nub");
assert.equal(pixels(".battery-contact.bottom-left", "top"), 73, "The lower positive lead must align with its nub");
assert.equal(pixels(".battery-contact.bottom-right", "top"), 71, "The lower spring must align with the AA barrel");
const lidSlot = block(".battery-lid-slot");
assert.match(lidSlot, /position:\s*absolute/, "The cover tab opening must stay attached to the battery bay");
assert.match(lidSlot, /left:\s*50%/, "The cover tab opening must stay centered");
assert.match(lidSlot, /background:/, "The cover tab receptacle must remain visibly open");
assert.match(css, /(?:^|\n)\.rear-brand\s*\{[^}]*display:\s*none/s, "The reference cover has no embossed wordmark");
const batteryLabel = block(".aa-battery-label");
assert.match(batteryLabel, /font-family:\s*[^;]*sans-serif/, "Battery labels must use smooth product typography");
assert.match(batteryLabel, /-webkit-font-smoothing:\s*antialiased/, "Battery labels must be anti-aliased");
assert.match(batteryLabel, /image-rendering:\s*auto/, "Battery labels must opt out of the pixel-art rendering rule");
assert.match(batteryLabel, /border-radius:\s*3px/, "The printed wrapper must preserve the squared reference silhouette");
assert.match(css, /\.aa-battery-label::after\s*\{[^}]*linear-gradient\(to bottom/s, "The wrapper needs a curved barrel highlight");
assert.match(
  block(".aa-battery.top .aa-battery-label"),
  /rotate\(180deg\)/,
  "The reference's top battery label must be upside down",
);
assert.match(css, /\.battery-pack\.codex[\s\S]*?#2459e0/, "Codex batteries must use the blue brand treatment");
assert.match(css, /\.battery-pack\.claude[\s\S]*?#d97757/i, "Claude batteries must use the coral brand treatment");
assert.deepEqual(
  backgroundColors(".battery-pack.codex .aa-battery-label"),
  ["#2459e0", "#111217"],
  "Installed Codex batteries must use the blue/black two-tone treatment",
);
assert.deepEqual(
  backgroundColors(".battery-choice.codex"),
  ["#2459e0", "#111217"],
  "The Codex battery choice must match the installed two-tone treatment",
);
assert.deepEqual(
  backgroundColors(".battery-pack.claude .aa-battery-label"),
  ["#d97757", "#f0eee6"],
  "Installed Claude batteries must use the orange/cream two-tone treatment",
);
assert.deepEqual(
  backgroundColors(".battery-choice.claude"),
  ["#d97757", "#f0eee6"],
  "The Claude battery choice must match the installed two-tone treatment",
);
for (const selector of [
  ".battery-pack.codex .aa-battery-label",
  ".battery-pack.claude .aa-battery-label",
]) {
  assert.match(
    block(selector),
    /radial-gradient\(ellipse 4px 50% at calc\(28% - 4px\) 50%/,
    `${selector} must curve its colored wrapper band around the cylinder`,
  );
}
for (const [selector, color, neutral] of [
  [".battery-choice.codex", "#2459e0", "#111217"],
  [".battery-choice.claude", "#d97757", "#f0eee6"],
]) {
  assert.match(
    block(selector),
    new RegExp(`${color} 0 28%,\\s*${neutral} 28% 100%`, "i"),
    `${selector} must use the same 28% colored section`,
  );
}
assert.doesNotMatch(css, /\.battery-logo|\.codex-logo-mark|\.codex-cloud|\.codex-terminal-mark/, "Removed Codex artwork must not leave a drawn arc or logo");
const rearLatch = block(".rear-latch");
assert.match(rearLatch, /top:\s*-13px/, "The removable cover lever must straddle its top edge");
assert.doesNotMatch(rearLatch, /bottom:/, "The cover lever must not drift back to the bottom edge");
assert.match(css, /@keyframes powerIndicatorRejected/, "Rejected power-on attempts need an LED flash sequence");
assert.doesNotMatch(css, /#power-switch\.power-rejected/, "The physical power toggle must never flash red");
assert.match(css, /prefers-reduced-motion:[\s\S]*?\.power-led\.rejected/s, "The rejection LED needs a reduced-motion state");

assert.match(adapter, /const PROVIDER_STORAGE_KEY = "cqa-ai-provider"/, "Provider selection must persist locally");
assert.match(adapter, /invoke\("engine_set_ai_provider"/, "Battery changes must reach the backend");
assert.match(adapter, /invoke\("verify_ai_provider"/, "Power-on must prove the selected provider works");
assert.match(
  adapter,
  /await verifyInstalledProvider\(\)[\s\S]*?invoke\("engine_power", \{ powered: true \}\)/,
  "The engine must not power on until provider verification succeeds",
);
assert.match(adapter, /function rejectPowerOn\(/, "Missing the failed power-on recovery path");
assert.doesNotMatch(adapter, /OPEN_DEVICE_HEIGHT|battery-door-open/, "The open cover must not affect native fitting");
assert.match(adapter, /window\.innerHeight \/ DEVICE_HEIGHT/, "Device fitting must use one stable height");
assert.match(adapter, /powerLed\.classList\.add\("rejected"\)/, "The power indicator never receives its red failure state");
assert.doesNotMatch(
  adapter,
  /powerSwitch\.classList\.(?:add|remove)\([^)]*power-rejected/,
  "Rejected power feedback must not recolor the physical toggle",
);
assert.match(adapter, /batteryDoor\.addEventListener\("click"/, "The rear battery door is not interactive");
assert.match(
  adapter,
  /batteryLidSlot\.addEventListener\("click"[\s\S]*?setBatteryDoorOpen\(false\)/,
  "The stationary tab opening must let the user replace the cover",
);
assert.match(adapter, /batteryLidSlot\.inert = !batteryDoorOpen/, "The cover return control must only activate while open");

assert.match(externalTools, /CQA_CODEX/, "Codex executable discovery must be configurable");
assert.match(rust, /fn engine_set_ai_provider\(/, "The Tauri boundary cannot accept installed batteries");
assert.match(rust, /fn verify_ai_provider\(/, "The Tauri boundary cannot verify a battery provider");
assert.match(rust, /AiProviderState/, "Verified provider state must be shared with question generation");
assert.match(
  rust,
  /fn engine_power\([\s\S]*?provider_state: State<AiProviderState>[\s\S]*?ready_provider\(\)\.is_none\(\)/,
  "The backend power boundary must reject an unverified provider",
);
assert.match(engine, /AiProvider\(Option<String>\)/, "The engine cannot render provider-aware status");
assert.doesNotMatch(
  engine.split("#[cfg(test)]")[0],
  /"CLAUDE:(?:READY|SCRYING|CLOUDY|CHANNEL)"/,
  "Engine status must not be hard-coded to Claude",
);

console.log("Provider battery contract OK: physical selection, verified boot gate, Codex/Claude runtime");
