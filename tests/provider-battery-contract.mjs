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

assert.match(block(".battery-compartment"), /position:\s*absolute/, "The battery bay must belong to the rear shell");
assert.match(
  block(".battery-compartment.open .rear-battery-door"),
  /transform:\s*translateY\(/,
  "Opening the compartment must move the physical door",
);
const batteryLabel = block(".aa-battery-label");
assert.match(batteryLabel, /font-family:\s*[^;]*sans-serif/, "Battery labels must use smooth product typography");
assert.match(batteryLabel, /-webkit-font-smoothing:\s*antialiased/, "Battery labels must be anti-aliased");
assert.match(batteryLabel, /image-rendering:\s*auto/, "Battery labels must opt out of the pixel-art rendering rule");
assert.match(
  block(".aa-battery.bottom .aa-battery-label"),
  /rotate\(180deg\)/,
  "One battery label must be upside down",
);
assert.match(css, /\.battery-pack\.codex[\s\S]*?#2459e0/, "Codex batteries must use the blue brand treatment");
assert.match(css, /\.battery-pack\.claude[\s\S]*?#d95f3f/, "Claude batteries must use the coral brand treatment");
assert.match(css, /@keyframes powerRejected/, "Rejected power-on attempts need a red flash sequence");
assert.match(css, /prefers-reduced-motion:[\s\S]*?power-rejected/s, "The rejection feedback needs a reduced-motion state");

assert.match(adapter, /const PROVIDER_STORAGE_KEY = "cqa-ai-provider"/, "Provider selection must persist locally");
assert.match(adapter, /invoke\("engine_set_ai_provider"/, "Battery changes must reach the backend");
assert.match(adapter, /invoke\("verify_ai_provider"/, "Power-on must prove the selected provider works");
assert.match(
  adapter,
  /await verifyInstalledProvider\(\)[\s\S]*?invoke\("engine_power", \{ powered: true \}\)/,
  "The engine must not power on until provider verification succeeds",
);
assert.match(adapter, /function rejectPowerOn\(/, "Missing the failed power-on recovery path");
assert.match(adapter, /power-rejected/, "The power switch never receives its red failure state");
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
