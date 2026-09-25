import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  DEVICE_MESSAGE_MAX_LENGTH,
  GUIDE_MESSAGE_MAX_LENGTH,
  deviceMessageText,
  leavesKeyToFocusedControl,
  trappedFocusTarget,
} from "../src/device-shell.js";
import { isRefusedCartridgeError, rackFocusIndex } from "../src/cartridge-library.js";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");

function fn(name) {
  const start = adapter.search(new RegExp(`\\n  (?:async )?function ${name}\\(`));
  assert.ok(start >= 0, `Missing function ${name}`);
  const end = adapter.indexOf("\n  }\n", start);
  assert.ok(end > start, `Unterminated function ${name}`);
  return adapter.slice(start, end + 4);
}

function listener(target, type) {
  const escaped = target.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = adapter.match(
    new RegExp(`${escaped}\\.addEventListener\\("${type}", \\(event\\) => \\{[\\s\\S]*?\\n    \\}\\);`),
  );
  assert.ok(match, `Missing ${type} listener on ${target}`);
  return match[0];
}

// 18: Enter activates a focused shell control instead of becoming START.
assert.equal(leavesKeyToFocusedControl("Enter", true), true);
assert.equal(leavesKeyToFocusedControl("NumpadEnter", true), true);
assert.equal(leavesKeyToFocusedControl("Enter", false), false, "Enter on the page body must stay START");
assert.equal(leavesKeyToFocusedControl("KeyD", true), false, "Game keys must keep reaching the engine");
const keydown = adapter.match(/window\.addEventListener\("keydown", \(event\) => \{[\s\S]*?\n  \}\);/)?.[0] || "";
assert.ok(keydown, "Missing the device keydown handler");
assert.ok(
  keydown.indexOf("leavesKeyToFocusedControl(event.code, isShellControl(event.target))")
    < keydown.indexOf("const button = keyMap[event.code]")
    && keydown.includes("leavesKeyToFocusedControl("),
  "Focused shell controls must keep Enter before the START mapping cancels it",
);
assert.match(fn("isShellControl"), /button, \[role=button\], \[role=switch\]/);
assert.match(
  adapter,
  /viewToggle\.addEventListener\("pointerdown", \(event\) => event\.preventDefault\(\)\)/,
  "Mouse use of the FRONT/BACK toggle must not capture Enter from START",
);

// 19: key repeat never re-toggles power or re-inserts a cartridge.
const switchKeydown = adapter.match(
  /switchControl\.addEventListener\("keydown", \(event\) => \{[\s\S]*?\n    \}\);/,
)?.[0] || "";
assert.match(switchKeydown, /if \(event\.repeat\) return;\s*setPower\(!powered\)/, "Held power keys must toggle once");
assert.match(listener("card", "keydown"), /^[^]*?\{\s*if \(event\.repeat\) return;/, "Held cart keys must act once");
const insertCartridge = fn("insertCartridge");
assert.match(insertCartridge, /if \(!value \|\| cartridge \|\| inserting\) return;/);
assert.match(insertCartridge, /inserting = true;[\s\S]*?finally \{\s*inserting = false;/);
const checkProvider = fn("checkProvider");
assert.match(
  checkProvider,
  /if \(providerCheck\?\.provider === provider\) return providerCheck\.result;/,
  "A new power-on must join an abandoned cycle's in-flight CLI probe instead of starting another",
);
assert.match(checkProvider, /invoke\("verify_ai_provider", \{ provider \}\)/);

// 20: the real power-on failure reaches the device and both battery guides.
assert.equal(deviceMessageText("CODEX CLI UNAVAILABLE"), "CODEX CLI UNAVAILABLE");
assert.equal(deviceMessageText(new Error("RUN IN TAURI TO LOAD CARTRIDGES")), "RUN IN TAURI TO LOAD CARTRIDGES");
assert.equal(deviceMessageText({ message: "CLAUDE CALL TIMED OUT" }), "CLAUDE CALL TIMED OUT");
assert.equal(deviceMessageText("  INVALID CODEQUEST.toml:\n  line 3  "), "INVALID CODEQUEST.toml: line 3");
assert.equal(deviceMessageText(""), "UNKNOWN FAULT");
assert.equal(deviceMessageText(null, { fallback: "CODEX NOT READY" }), "CODEX NOT READY");
assert.equal(deviceMessageText("X".repeat(400)).length, DEVICE_MESSAGE_MAX_LENGTH);
assert.ok(deviceMessageText("X".repeat(400)).endsWith("…"));
assert.equal(
  deviceMessageText("Y".repeat(80), { maxLength: GUIDE_MESSAGE_MAX_LENGTH }).length,
  GUIDE_MESSAGE_MAX_LENGTH,
);
assert.match(fn("powerFailureReason"), /deviceMessageText\(error, \{[\s\S]*?NOT READY/);
const rejectPowerOn = fn("rejectPowerOn");
assert.match(rejectPowerOn, /lastPowerFailure = powerFailureReason\(message\);/, "Keep the backend's failure reason");
assert.match(rejectPowerOn, /showDeviceError\(lastPowerFailure\)/, "Show the failure reason on the device");
const guides = fn("updateControlGuides");
assert.match(guides, /const batteryFailure = installedProvider \? lastPowerFailure : "";/);
assert.match(
  guides,
  /const needsBatteryTab =[\s\S]*?\(!installedProvider \|\| Boolean\(batteryFailure\)\)[\s\S]*?!batteryDoorOpen/,
  "Installed-but-failed batteries must guide the back face to the battery tab",
);
assert.match(
  guides,
  /const needsBatteryBay =[\s\S]*?\(!installedProvider \|\| Boolean\(batteryFailure\)\)[\s\S]*?batteryDoorOpen/,
  "Installed-but-failed batteries must light the bay once the cover is off",
);
const batteryGuides = fn("renderBatteryGuides");
assert.match(batteryGuides, /viewGuide\.querySelector\("\.guide-action"\)\.textContent = failure \|\| "CHECK BATTERIES"/);
assert.match(batteryGuides, /batteryGuide\.querySelector\("\.guide-action"\)\.textContent = failure \|\| "OPEN BATTERY TAB"/);
assert.match(fn("renderBatteryTray"), /`\$\{lastPowerFailure\} · EJECT TO SWAP`/);

// 21: transient load faults keep rack entries and the saved battery choice.
assert.equal(isRefusedCartridgeError("NOT A GIT REPOSITORY - CARTRIDGE REFUSED"), true);
assert.equal(isRefusedCartridgeError(new Error("NOT A GIT REPOSITORY")), true);
for (const transient of [
  "INVALID CODEQUEST.toml: expected `=`",
  "DIRECTORY NOT FOUND",
  "BEVY ENGINE STOPPED",
  "CANNOT READ CODEQUEST.toml: denied",
  null,
]) {
  assert.equal(isRefusedCartridgeError(transient), false, `${transient} must not recycle a cartridge`);
}
const insertByPath = fn("insertByPath");
assert.match(
  insertByPath,
  /if \(isRefusedCartridgeError\(error\)\) \{\s*forgetCartridge\(path\);/,
  "A failed load may forget only a folder that is no longer a repository",
);
assert.match(insertByPath, /showDeviceError\(error\)/);
const initialize = fn("initialize");
assert.doesNotMatch(
  initialize,
  /removeItem\(PROVIDER_STORAGE_KEY\);\s*renderProviderBatteries\(\);\s*show/,
  "An engine error at startup must not erase the saved battery choice",
);
assert.match(initialize, /if \(isRefusedCartridgeError\(error\)\) forgetCartridge\(savedPath\);/);
assert.match(initialize, /showDeviceError\(`CARTRIDGE NOT LOADED · \$\{deviceMessageText\(error\)\}`\)/);
assert.doesNotMatch(initialize, /catch \(_\) \{\s*cartridge = null/, "Startup cartridge faults must not be swallowed");

// 22: abandoned power-on attempts never touch the battery UI.
const verifyInstalledProvider = fn("verifyInstalledProvider");
assert.match(verifyInstalledProvider, /const current = \(\) => generation === powerGeneration;/);
assert.match(verifyInstalledProvider, /if \(!current\(\)\) return;\s*verifiedProvider = provider;/);
assert.match(verifyInstalledProvider, /if \(current\(\)\) \{\s*verifiedProvider = null;/);
assert.match(verifyInstalledProvider, /finally \{\s*if \(current\(\)\) batteryCompartment\.classList\.remove\("checking"\);/);
const setPower = fn("setPower");
assert.match(
  setPower,
  /await waitForPaint\(\);\s*if \(generation !== powerGeneration \|\| !powered\) return;\s*await verifyInstalledProvider\(generation\);/,
  "A power-on abandoned during the first paint must not start a probe",
);
assert.match(
  setPower,
  /batteryCompartment\.classList\.remove\("locked", "checking"\);\s*hideDeviceBoot\(\);\s*renderProviderBatteries\(\);/,
  "Power-off must clear a stale CHECKING state",
);

// 23: one visible, announced device message surface outside the trays.
assert.match(
  html,
  /<div id="device-message" role="status" aria-live="polite" aria-atomic="true"><\/div>/,
  "The device needs a visible live status surface",
);
assert.ok(
  html.indexOf('id="device-message"') < html.indexOf('id="cart-tray"')
    && html.indexOf('id="device-message"') > html.indexOf('id="shell-scale"'),
  "The device message must live outside the cartridge tray and the inert device",
);
for (const id of ["tray-error", "battery-tray-error"]) {
  assert.match(
    html,
    new RegExp(`<div id="${id}" role="status" aria-live="polite" aria-atomic="true"><\\/div>`),
    `${id} must be an announced message line that stays rendered`,
  );
}
assert.doesNotMatch(adapter, /showTray(?:Error|Message)\(/, "Messages must route through the device surface");
assert.doesNotMatch(
  adapter,
  /setBatteryStatus\("(?:TURN POWER OFF|EJECT INSTALLED|BATTERY CONTACT)/,
  "Refusals are device messages, not a second announcement from the battery state region",
);
const surface = fn("activeMessageSurface");
assert.match(surface, /if \(trayOpen\) return trayError;\s*if \(batteryTrayOpen\) return batteryTrayError;\s*return deviceMessage;/);
assert.match(fn("hideDeviceMessage"), /messageSurface\.textContent = ""/, "Hidden messages must leave no stale text");
assert.match(fn("showDeviceMessage"), /if \(!persist\) messageTimer = window\.setTimeout\(hideDeviceMessage, MESSAGE_DURATION_MS\)/);
assert.match(
  adapter,
  /initialize\(\)\.catch\(\(error\) => \{\s*showDeviceError\(`DEVICE FAULT · \$\{deviceMessageText\(error\)\}`, \{ persist: true \}\);/,
  "A failed startup must leave a visible fault instead of a blank device",
);
assert.match(fn("finishDeviceBoot"), /showDeviceError\(error\)/);
const deviceMessageCss = css.match(/#device-message \{([\s\S]*?)\}/)?.[1] || "";
assert.match(deviceMessageCss, /position:\s*fixed/);
assert.match(deviceMessageCss, /z-index:\s*55/, "Device messages must sit above the tray dimmers");
assert.match(deviceMessageCss, /font:\s*8px\/1\.6 'Press Start 2P'/, "Device messages must use the shell typeface");
assert.match(deviceMessageCss, /pointer-events:\s*none/);
assert.match(css, /#device-message:empty \{ opacity: 0; \}/);
assert.doesNotMatch(css, /#tray-error\.hidden/, "Live message lines must not be removed from the accessibility tree");

// 24: both trays are real modals.
assert.match(
  html,
  /id="cart-tray"[^>]*role="dialog"[^>]*aria-modal="true"[^>]*aria-labelledby="cart-tray-head"/,
  "The cartridge rack must open as a labelled modal dialog",
);
assert.match(html, /id="cart-tray-head" class="tray-head"/);
assert.match(fn("buildTray"), /\$\("cart-tray-head"\)\.textContent =/);
assert.equal(trappedFocusTarget([], null), null);
const [first, middle, last] = ["first", "middle", "last"];
const focusables = [first, middle, last];
assert.equal(trappedFocusTarget(focusables, "body"), first, "Tab from outside the dialog must enter at the start");
assert.equal(trappedFocusTarget(focusables, "body", true), last, "Shift+Tab from outside must enter at the end");
assert.equal(trappedFocusTarget(focusables, last), first, "Tab must wrap at the end");
assert.equal(trappedFocusTarget(focusables, first, true), last, "Shift+Tab must wrap at the start");
assert.equal(trappedFocusTarget(focusables, middle), null, "Inner Tab moves stay native");
const modality = fn("syncTrayModality");
assert.match(modality, /const modal = trayOpen \|\| batteryTrayOpen;/);
for (const target of ["scaleEl", "viewToggle", "viewGuide"]) {
  assert.match(modality, new RegExp(`${target}\\.inert = modal;`), `${target} must be inert behind an open tray`);
}
assert.match(fn("trapTabFocus"), /trappedFocusTarget\(focusables, document\.activeElement, event\.shiftKey\)/);
assert.match(keydown, /trapTabFocus\(event, batteryOptions\)/);
assert.match(keydown, /if \(trayOpen && event\.key === "Tab"\) \{\s*trapTabFocus\(event, cartTray\);/);
const openTray = fn("openTray");
assert.match(openTray, /trayReturnFocus = /, "Opening the rack must remember the previous focus");
assert.match(openTray, /syncTrayModality\(\);\s*\$\("tray-carts"\)\.querySelector\("button:not\(:disabled\)"\)\?\.focus\(\);/);
assert.match(openTray, /cartTray\.setAttribute\("aria-hidden", "false"\)/);
const closeTray = fn("closeTray");
assert.match(closeTray, /syncTrayModality\(\);\s*if \(focusWasInTray\) trayReturnFocus\?\.focus\(\);/);
assert.match(closeTray, /cartTray\.setAttribute\("aria-hidden", "true"\)/);
assert.match(fn("openBatteryTray"), /syncTrayModality\(\);\s*focusBatteryTray\(\);/);
assert.match(fn("closeBatteryTray"), /syncTrayModality\(\);\s*if \(restoreFocus/, "Focus must return only after the device is interactive again");
assert.match(adapter, /else if \(batteryTrayOpen\) focusBatteryTray\(\);/, "A failed battery change must re-focus the tray");
assert.match(
  adapter,
  /\$\("cart-back"\)\.addEventListener\("pointerdown", \(event\) => \{\s*event\.preventDefault\(\);/,
  "Opening the rack by pointer must not immediately blur its focused card",
);

assert.equal(rackFocusIndex(["/a", "/b", undefined], "/b", 0), 1, "A rebuilt rack keeps focus on the same cartridge");
assert.equal(rackFocusIndex(["/a", undefined], "/b", 1), 1, "A recycled card hands focus to its neighbor");
assert.equal(rackFocusIndex(["/a"], null, 4), 0, "Focus clamps to the last remaining card");
assert.equal(rackFocusIndex(["/a", undefined], null, null), -1, "Unfocused racks stay unfocused");
assert.equal(rackFocusIndex([undefined], undefined, null), -1, "Action tiles never match a missing path");
assert.equal(rackFocusIndex([], "/a", 0), -1);
const buildTray = fn("buildTray");
assert.match(buildTray, /card\.dataset\.path = value\.path;/);
assert.match(buildTray, /rackFocusIndex\(cards\.map\(\(card\) => card\.dataset\.path\), focusPath, restoreIndex\)/);
assert.match(fn("recycleCartridge"), /if \(trayOpen\) buildTray\(\{ focusIndex \}\);/);

// 25: branch refresh relabels cards in place.
const refresh = fn("refreshCartridgeBranches");
assert.doesNotMatch(refresh, /buildTray\(/, "A background refresh must not rebuild the open rack");
assert.match(refresh, /relabelTrayCard\(value\)/);
const relabel = fn("relabelTrayCard");
assert.match(relabel, /element\.dataset\.path === value\.path/);
assert.match(relabel, /\.cc-sub"\)\.textContent = value\.branch/);
assert.match(relabel, /setAttribute\("aria-label", cartridgeCardLabel\(/);
assert.doesNotMatch(buildTray, /tray-error|hideDeviceMessage/, "Rebuilding the rack must not hide a fresh message");

// 26: the browser demo explains why ADD FROM DISK cannot load.
assert.match(
  fn("createBrowserDemo"),
  /if \(command === "pick_cartridge"\) throw new Error\("RUN IN TAURI TO LOAD CARTRIDGES"\);/,
);

console.log("Device shell robustness contract OK: focus, repeat, fault surfacing, modal trays, stable rack");
