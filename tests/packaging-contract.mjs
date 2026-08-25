import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const workflow = readFileSync(new URL("../.github/workflows/package.yml", import.meta.url), "utf8");
const appImageBuilder = readFileSync(
  new URL("../packaging/build-appimage.sh", import.meta.url),
  "utf8",
);
const appImageContainer = readFileSync(
  new URL("../packaging/Containerfile", import.meta.url),
  "utf8",
);
const linuxArtifactTest = readFileSync(
  new URL("../scripts/test-release-picker.sh", import.meta.url),
  "utf8",
);
const macosArtifactTest = readFileSync(
  new URL("../scripts/test-macos-packages.sh", import.meta.url),
  "utf8",
);
const windowsArtifactTest = readFileSync(
  new URL("../scripts/test-windows-packages.ps1", import.meta.url),
  "utf8",
);
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
assert.match(workflow, /--bundles app,dmg --target universal-apple-darwin/);
assert.match(workflow, /runs-on: macos-15/);
assert.match(workflow, /^\s*pull_request:/m);
assert.match(workflow, /^\s*workflow_dispatch:/m);
assert.match(workflow, /^\s*push:\n\s+tags:\n\s+- "v\*"/m);
assert.equal([...workflow.matchAll(/if-no-files-found: error/g)].length, 3);
assert.equal([...workflow.matchAll(/actions\/download-artifact@v4/g)].length, 4);
assert.match(workflow, /name: Test uploaded Linux AppImage/);
assert.match(workflow, /x11-apps/);
assert.match(workflow, /name: Test uploaded Windows installers/);
assert.match(workflow, /name: Test uploaded macOS packages/);
assert.match(workflow, /needs: \[test-linux, test-windows, test-macos\]/);
assert.match(
  appImageBuilder,
  /--build-arg\s+"CQA_BUILD_REVISION=\$build_revision"/,
  "The container build must receive the checked-out app revision",
);
assert.match(
  appImageContainer,
  /ARG CQA_BUILD_REVISION[\s\S]*?RUN npm run tauri build -- --bundles appimage/,
  "The AppImage build must expose the app revision to Cargo",
);

assert.match(linuxArtifactTest, /--appimage-extract-and-run/);
assert.match(linuxArtifactTest, /SELECT CARTRIDGE \(GIT REPO\)/);
assert.match(linuxArtifactTest, /xdotool mousemove 512 112 click 1/);
assert.doesNotMatch(linuxArtifactTest, /xdotool key c/);
assert.match(windowsArtifactTest, /msiexec\.exe/);
assert.match(windowsArtifactTest, /\/a .*\/qn TARGETDIR=/);
assert.match(windowsArtifactTest, /\/S \/D=/);
assert.match(windowsArtifactTest, /Test-ApplicationStartup/);
assert.match(macosArtifactTest, /shasum -a 256 -c/);
assert.match(macosArtifactTest, /codesign --verify --deep --strict/);
assert.match(macosArtifactTest, /lipo -archs/);
assert.match(macosArtifactTest, /hdiutil attach/);

assert.equal(packageJson.scripts.tauri, "tauri");
assert.ok(tauriConfig.bundle.icon.includes("icons/icon.icns"));
assert.ok(tauriConfig.bundle.icon.includes("icons/icon.ico"));

assert.match(cargo, /tauri-plugin-dialog/);
assert.match(cargo, /wait-timeout/);
assert.doesNotMatch(rust, /Command::new\("zenity"\)/);
assert.doesNotMatch(rust, /Command::new\("timeout"\)/);
assert.match(externalTools, /CQA_GIT/);
assert.match(externalTools, /CQA_CLAUDE/);
assert.match(externalTools, /CQA_CODEX/);
assert.match(externalTools, /CQA_SHELL/);
assert.match(externalTools, /windows_git_bash/);
assert.match(externalTools, /is_windows_subsystem_launcher/);
assert.match(externalTools, /CREATE_NO_WINDOW/);
assert.match(externalTools, /creation_flags\(CREATE_NO_WINDOW\)/);

console.log("packaging contract: ok");
