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

for (const id of [
  "battery-compartment",
  "battery-bay",
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
assert.equal(
  [...html.matchAll(/class="codex-logo-mark"/g)].length,
  2,
  "Both provider cells must carry the smooth Codex terminal-cloud mark",
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
assert.ok(
  pixels("#shell-scale.battery-door-open", "height") >= 446,
  "The open-device fit box must include the fully removed cover",
);
assert.match(
  block(".battery-compartment.open .rear-battery-door"),
  /transform:\s*translateY\(/,
  "Opening the compartment must move the physical door",
);
assert.doesNotMatch(
  block(".battery-compartment.open .rear-battery-door"),
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
assert.match(batteryBody, /border-radius:\s*12px/, "AA cells need straight cylindrical barrels with shallow shoulders");
assert.match(block(".battery-contact.spring"), /overflow:\s*visible/, "The conical springs must remain fully visible");
assert.match(css, /\.spring-coil\s*\{[^}]*stroke:/s, "The conical spring wire must be visibly drawn");
assert.match(block(".battery-contact.leaf"), /background:/, "The flat electrical leads must be visible");
assert.ok(pixels(".battery-pack", "left") <= 30, "Reference AA cells must reach the compact end contacts");
assert.ok(pixels(".battery-pack", "width") >= 260, "Reference AA cells must span the molded compartment");
assert.match(css, /(?:^|\n)\.rear-brand\s*\{[^}]*display:\s*none/s, "The reference cover has no embossed wordmark");
const batteryLabel = block(".aa-battery-label");
assert.match(batteryLabel, /font-family:\s*[^;]*sans-serif/, "Battery labels must use smooth product typography");
assert.match(batteryLabel, /-webkit-font-smoothing:\s*antialiased/, "Battery labels must be anti-aliased");
assert.match(batteryLabel, /image-rendering:\s*auto/, "Battery labels must opt out of the pixel-art rendering rule");
assert.match(batteryLabel, /border-radius:\s*11px/, "The printed wrapper must follow the cylindrical barrel");
assert.match(css, /\.aa-battery-label::after\s*\{[^}]*linear-gradient\(to bottom/s, "The wrapper needs a curved barrel highlight");
assert.match(
  block(".aa-battery.bottom .aa-battery-label"),
  /rotate\(180deg\)/,
  "One battery label must be upside down",
);
assert.match(css, /\.battery-pack\.codex[\s\S]*?#2459e0/, "Codex batteries must use the blue brand treatment");
assert.match(css, /\.battery-pack\.claude[\s\S]*?#d97757/i, "Claude batteries must use the coral brand treatment");
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
assert.match(adapter, /const OPEN_DEVICE_HEIGHT = 446/, "The open cover must participate in native window fitting");
assert.match(
  adapter,
  /scaleEl\.classList\.toggle\("battery-door-open", batteryDoorOpen && shellBackVisible\)/,
  "The native fit box must expand only while the back cover is open",
);
assert.match(adapter, /powerLed\.classList\.add\("rejected"\)/, "The power indicator never receives its red failure state");
assert.doesNotMatch(
  adapter,
  /powerSwitch\.classList\.(?:add|remove)\([^)]*power-rejected/,
  "Rejected power feedback must not recolor the physical toggle",
);
assert.match(adapter, /batteryDoor\.addEventListener\("click"/, "The rear battery door is not interactive");

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
