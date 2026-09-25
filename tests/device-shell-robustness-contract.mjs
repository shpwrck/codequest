import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  DEVICE_MESSAGE_MAX_LENGTH,
  GUIDE_MESSAGE_MAX_LENGTH,
  deviceMessageText,
  leavesKeyToFocusedControl,
  trappedFocusTarget,
} from "../src/device-shell.js";
import {
  CARTRIDGE_REFUSED_MESSAGE,
  isRefusedCartridgeError,
  rackFocusIndex,
} from "../src/cartridge-library.js";
import { bootShell, rackStorage } from "./shell-harness.mjs";

const html = readFileSync(new URL("../src/index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const adapter = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const backend = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");

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
const focusGuardAt = keydown.search(
  /if \(leavesKeyToFocusedControl\(event\.code, isShellControl\(event\.target\)\)\) return;/,
);
const startMappingAt = keydown.indexOf("const button = keyMap[event.code]");
assert.ok(focusGuardAt >= 0, "The keydown handler must leave Enter to a focused shell control");
assert.ok(startMappingAt >= 0, "The keydown handler must map keys to device buttons");
assert.ok(
  focusGuardAt < startMappingAt,
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
  deviceMessageText("Z".repeat(DEVICE_MESSAGE_MAX_LENGTH)),
  "Z".repeat(DEVICE_MESSAGE_MAX_LENGTH),
  "A message that exactly fits the device line is shown whole",
);
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
assert.equal(CARTRIDGE_REFUSED_MESSAGE, "NOT A GIT REPOSITORY - CARTRIDGE REFUSED");
assert.equal(isRefusedCartridgeError(CARTRIDGE_REFUSED_MESSAGE), true);
assert.equal(isRefusedCartridgeError(new Error(CARTRIDGE_REFUSED_MESSAGE)), true);
assert.equal(isRefusedCartridgeError({ message: CARTRIDGE_REFUSED_MESSAGE }), true);
for (const transient of [
  "INVALID CODEQUEST.toml: expected `=`",
  "DIRECTORY NOT FOUND",
  "BEVY ENGINE STOPPED",
  "CANNOT READ CODEQUEST.toml: denied",
  // Git that could not answer says nothing about the folder (lib.rs
  // git_repo_check_within), and a damaged save is the player's to repair.
  "GIT CALL TIMED OUT",
  "GIT CLI UNAVAILABLE - NOT FOUND",
  "GIT CALL FAILED - FATAL: CANNOT CHANGE TO 'D:\\REPO': PERMISSION DENIED",
  "CARTRIDGE SAVE IS CORRUPT",
  "GIT CALL FAILED: program not found",
  // The bare phrase is what a failed git probe used to collapse into.
  "NOT A GIT REPOSITORY",
  new Error("NOT A GIT REPOSITORY"),
  null,
]) {
  assert.equal(isRefusedCartridgeError(transient), false, `${transient} must not recycle a cartridge`);
}
// The shell and the backend must agree on the one refusal, and no other
// backend message may be mistaken for it.
const backendMessages = [...backend.matchAll(/"([A-Z][A-Z0-9 .:·'\/_-]{5,})"/g)].map(([, text]) => text);
assert.ok(
  backendMessages.includes(CARTRIDGE_REFUSED_MESSAGE),
  "The backend must still refuse non-repositories with the text the shell recycles on",
);
for (const message of backendMessages) {
  assert.equal(
    isRefusedCartridgeError(message),
    message === CARTRIDGE_REFUSED_MESSAGE,
    `Backend message ${message} must not recycle a cartridge`,
  );
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
assert.doesNotMatch(
  initialize,
  /catch \(error\) \{[^}]*localStorage\.removeItem\("cqa-cart-id"\)/,
  "A startup fault that may clear must keep the saved slot for the next launch",
);
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

/* ---------- The real adapter, driven through its own handlers ---------- */

const cartridgeFor = (path) => ({
  path,
  title: path.split("/").at(-1).toUpperCase(),
  branch: "main",
  revision: "abc1234",
  color: "#6a6fd1",
});
const rackPaths = (shell) => JSON.parse(shell.storage.getItem("cqa-repo-carts")).map(({ path }) => path);
const poweredOn = (shell) => shell.calledWith("engine_power", ({ powered }) => powered === true).length;

// 27: power never switches on under a cartridge load that is still walking git.
{
  const shell = await bootShell({ storage: rackStorage(["/repos/big"]) });
  await shell.key("KeyC");
  assert.ok(shell.trayOpen, "C opens the rack while powered off");
  shell.hold("engine_set_cartridge");
  await shell.dispatch(shell.rackCard(0), "keydown", { key: "Enter" });
  assert.ok(shell.isHeld("engine_set_cartridge"), "Enter on a rack card starts the load");
  await shell.key("KeyP");
  await shell.paint();
  assert.equal(shell.calledWith("verify_ai_provider").length, 0, "P during a cartridge load must not start a power-on");
  assert.equal(shell.powered, false, "The power switch stays off while the cartridge loads");
  assert.match(shell.messages, /WAIT FOR THE CARTRIDGE/, "A refused power-on says why");
  await shell.answer("engine_set_cartridge", cartridgeFor("/repos/big"));
  assert.ok(shell.loaded && !shell.trayOpen, "The finished load seats the cartridge and closes the rack");
  await shell.key("KeyP");
  await shell.paint();
  assert.equal(shell.powered, true, "Power-on works once the cartridge is seated");
  assert.equal(poweredOn(shell), 1);
  await shell.advance(2600);
  assert.equal(shell.calledWith("engine_finish_boot").length, 1, "The boot logo finishes on its own timer");
  shell.close();
}
{
  // The native folder picker is part of the same cartridge operation.
  const shell = await bootShell({ storage: rackStorage([]) });
  await shell.key("KeyC");
  shell.hold("pick_cartridge");
  await shell.dispatch(shell.rackAction("add"), "click");
  await shell.key("KeyP");
  await shell.paint();
  assert.equal(shell.calledWith("verify_ai_provider").length, 0, "P while the folder picker is open must not power on");
  await shell.answer("pick_cartridge", cartridgeFor("/repos/picked"));
  assert.ok(shell.loaded, "The picked cartridge still loads");
  assert.equal(shell.powered, false);
  shell.close();
}

// 27: ejecting is fenced the same way, and a double click unloads once.
{
  const shell = await bootShell({ storage: rackStorage(["/repos/big"], { current: "/repos/big" }) });
  assert.ok(shell.loaded, "The saved cartridge is seated at startup");
  await shell.key("KeyC");
  const eject = shell.rackAction("eject");
  await shell.dispatch(eject, "click");
  await shell.dispatch(eject, "click");
  await shell.key("KeyP");
  await shell.paint();
  assert.equal(shell.calledWith("verify_ai_provider").length, 0, "P during the eject animation must not power on");
  shell.hold("engine_set_cartridge");
  await shell.advance(240);
  assert.ok(shell.isHeld("engine_set_cartridge"), "The eject unloads the engine after its animation");
  await shell.key("KeyP");
  await shell.paint();
  assert.equal(shell.calledWith("verify_ai_provider").length, 0, "P during the unload must not power on");
  await shell.answer("engine_set_cartridge", null);
  await shell.advance(240);
  assert.equal(shell.loaded, false);
  assert.equal(
    shell.calledWith("engine_set_cartridge", ({ path }) => path === null).length,
    1,
    "A double-clicked eject unloads the engine once",
  );
  assert.equal(shell.storage.getItem("cqa-cart-id"), null);
  shell.close();
}

// 28: power input waits for initialize(), which then never switches it off.
{
  const shell = await bootShell({
    storage: rackStorage(["/repos/big"], { current: "/repos/big" }),
    hold: ["app_revision", "engine_set_cartridge"],
  });
  await shell.key("KeyP");
  await shell.paint();
  await shell.answer("app_revision", "abc1234");
  assert.ok(shell.isHeld("engine_set_cartridge"), "Startup is still loading the saved cartridge");
  await shell.dispatch(shell.el("power-switch"), "pointerdown");
  await shell.key("KeyC");
  await shell.paint();
  assert.equal(shell.calledWith("verify_ai_provider").length, 0, "Power input before startup finishes must be ignored");
  assert.equal(shell.powered, false);
  assert.equal(shell.trayOpen, false, "The rack cannot open under the startup load");
  await shell.answer("engine_set_cartridge", cartridgeFor("/repos/big"));
  assert.deepEqual(
    shell.calledWith("engine_power").map(({ args }) => args.powered),
    [false],
    "Startup switches the engine off exactly once",
  );
  assert.equal(
    shell.el("battery-status").textContent,
    "CODEX · UNTESTED",
    "An early P must not leave a stale NO BATTERIES failure behind",
  );
  await shell.key("KeyP");
  await shell.paint();
  assert.equal(shell.powered, true);
  assert.equal(shell.calledWith("engine_power").at(-1).args.powered, true, "Nothing switches the engine off afterwards");
  await shell.advance(2600);
  assert.equal(shell.calledWith("engine_finish_boot").length, 1, "The saved cartridge boots on its own timer");
  shell.close();
}

// 13: only the backend's refusal recycles a cartridge; git faults keep it.
{
  const shell = await bootShell({
    storage: rackStorage(["/repos/share", "/repos/gone"], { current: "/repos/share" }),
    hold: ["engine_set_cartridge"],
  });
  await shell.answer("engine_set_cartridge", new Error("GIT CALL TIMED OUT"));
  assert.match(shell.messages, /CARTRIDGE NOT LOADED · GIT CALL TIMED OUT/);
  assert.deepEqual(rackPaths(shell), ["/repos/share", "/repos/gone"], "A git timeout at startup keeps the rack entry");
  assert.equal(shell.storage.getItem("cqa-cart-id"), "/repos/share", "The next launch retries the saved cartridge");
  await shell.key("KeyC");
  shell.hold("engine_set_cartridge");
  await shell.dispatch(shell.rackCard(0), "keydown", { key: "Enter" });
  await shell.answer("engine_set_cartridge", new Error("GIT CALL FAILED: program not found"));
  assert.deepEqual(rackPaths(shell), ["/repos/share", "/repos/gone"], "Git that cannot start keeps the rack entry");
  shell.hold("engine_set_cartridge");
  await shell.dispatch(shell.rackCard(1), "keydown", { key: "Enter" });
  await shell.answer("engine_set_cartridge", new Error(CARTRIDGE_REFUSED_MESSAGE));
  assert.deepEqual(rackPaths(shell), ["/repos/share"], "A refused folder leaves the rack");
  assert.equal(shell.rackCard(0).dataset.path, "/repos/share", "The open rack is rebuilt without it");
  shell.close();
}
{
  const shell = await bootShell({
    storage: rackStorage(["/repos/plain"], { current: "/repos/plain" }),
    hold: ["engine_set_cartridge"],
  });
  await shell.answer("engine_set_cartridge", new Error(CARTRIDGE_REFUSED_MESSAGE));
  assert.deepEqual(rackPaths(shell), [], "A refused saved cartridge is forgotten at startup");
  assert.equal(shell.storage.getItem("cqa-cart-id"), null);
  shell.close();
}
{
  // A kept saved slot follows its rack entry when the player recycles it.
  const shell = await bootShell({
    storage: rackStorage(["/repos/share"], { current: "/repos/share" }),
    hold: ["engine_set_cartridge"],
  });
  await shell.answer("engine_set_cartridge", new Error("GIT CALL TIMED OUT"));
  await shell.key("KeyC");
  await shell.dispatch(shell.rackCard(0), "keydown", { key: "Delete" });
  await shell.advance(180);
  assert.deepEqual(rackPaths(shell), []);
  assert.equal(shell.storage.getItem("cqa-cart-id"), null, "A recycled cartridge must not come back next launch");
  shell.close();
}

// 29: what the rack learns is saved for the next launch.
{
  const shell = await bootShell({
    storage: rackStorage(["/repos/big"]),
    respond: { cartridge_branch: () => "story/next-chapter" },
  });
  await shell.key("KeyC");
  assert.equal(
    JSON.parse(shell.storage.getItem("cqa-repo-carts"))[0].branch,
    "story/next-chapter",
    "A refreshed branch label is saved with the rack",
  );
  await shell.dispatch(shell.rackCard(0), "keydown", { key: "Enter" });
  assert.ok(shell.loaded);
  assert.equal(shell.storage.getItem("cqa-cart-id"), "/repos/big", "A seated cartridge is reseated next launch");
  shell.close();
}

console.log("Device shell robustness contract OK: focus, repeat, fault surfacing, modal trays, stable rack");
