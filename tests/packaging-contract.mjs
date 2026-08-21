import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const workflow = readFileSync(new URL("../.github/workflows/package.yml", import.meta.url), "utf8");
const cargo = readFileSync(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8");
const rust = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const externalTools = readFileSync(
  new URL("../src-tauri/src/external_tools.rs", import.meta.url),
  "utf8",
);
const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
const tauriConfig = JSON.parse(
  readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);

assert.match(workflow, /runs-on: ubuntu-22\.04/);
assert.match(workflow, /--bundles msi,nsis/);
assert.match(workflow, /runs-on: windows-2025/);
assert.match(workflow, /Smoke-test Windows executable/);
assert.match(workflow, /Start-Process .* -WindowStyle Hidden/);
assert.match(workflow, /--bundles app,dmg --target universal-apple-darwin/);
assert.match(workflow, /runs-on: macos-15/);
assert.match(workflow, /pull_request:/);
assert.equal([...workflow.matchAll(/if-no-files-found: error/g)].length, 3);

assert.equal(packageJson.scripts.tauri, "tauri");
assert.ok(tauriConfig.bundle.icon.includes("icons/icon.icns"));
assert.ok(tauriConfig.bundle.icon.includes("icons/icon.ico"));

assert.match(cargo, /tauri-plugin-dialog/);
assert.match(cargo, /wait-timeout/);
assert.doesNotMatch(rust, /Command::new\("zenity"\)/);
assert.doesNotMatch(rust, /Command::new\("timeout"\)/);
assert.match(externalTools, /CQA_GIT/);
assert.match(externalTools, /CQA_CLAUDE/);
assert.match(externalTools, /CQA_SHELL/);
assert.match(externalTools, /windows_git_bash/);
assert.match(externalTools, /is_windows_subsystem_launcher/);
assert.match(externalTools, /CREATE_NO_WINDOW/);
assert.match(externalTools, /creation_flags\(CREATE_NO_WINDOW\)/);

console.log("packaging contract: ok");
