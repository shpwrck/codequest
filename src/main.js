/* CODE QUEST ADVANCE device adapter.
 * The browser owns only the shell. Bevy owns game state, timing, process
 * execution, the fixed 240x160 RGBA framebuffer, and every sound the speaker
 * plays; the shell only forwards input and presents what Rust emits. */
import {
  CARTRIDGE_DRAG_THRESHOLD,
  MAX_CARTRIDGES,
  cartridgeDragIntent,
  isRefusedCartridgeError,
  normalizeCartridges,
  rackFocusIndex,
  upsertCartridge,
} from "./cartridge-library.js";
import {
  GUIDE_MESSAGE_MAX_LENGTH,
  createWheelDetents,
  deviceMessageText,
  leavesKeyToFocusedControl,
  trappedFocusTarget,
} from "./device-shell.js";
import {
  VOLUME_LABELS,
  VOLUME_LEVELS,
  createSpeaker,
  nextVolume,
  stepVolume,
} from "./speaker.js";

"use strict";

(() => {
  const WIDTH = 240;
  const HEIGHT = 160;
  const FRAME_BYTES = WIDTH * HEIGHT * 4;
  const DEVICE_WIDTH = 618;
  const DEVICE_HEIGHT = 368;
  const BOOT_DURATION_MS = 2600;
  const BOOT_SKIP_DELAY_MS = 650;
  const TURN_DURATION_MS = 520;
  const POWER_REJECTION_MS = 760;
  const MESSAGE_DURATION_MS = 4000;
  const TRANSCRIPT_POLL_MS = 150;
  const PROVIDER_STORAGE_KEY = "cqa-ai-provider";
  const PROVIDERS = Object.freeze({
    codex: { label: "CODEX" },
    claude: { label: "CLAUDE" },
  });
  const $ = (id) => document.getElementById(id);
  const tauri = window.__TAURI__;
  const invoke = tauri?.core?.invoke
    ? (command, args) => tauri.core.invoke(command, args)
    : createBrowserDemo();

  const scaleEl = $("shell-scale");
  const frontFace = $("device-front");
  const backFace = $("device-back");
  const canvas = $("engine-canvas");
  const bootOverlay = $("device-boot");
  const cartGuide = $("cart-guide");
  const powerGuide = $("power-guide");
  const batteryGuide = $("battery-guide");
  const viewGuide = $("view-guide");
  const viewToggle = $("device-view-toggle");
  const rearSerial = $("rear-serial");
  const batteryCompartment = $("battery-compartment");
  const batteryBay = $("battery-bay");
  const batteryDoor = $("battery-door");
  const batteryLidSlot = $("battery-lid-slot");
  const batteryPack = $("battery-pack");
  const batteryChooser = $("battery-chooser");
  const batteryStatus = $("battery-status");
  const batteryTray = $("battery-tray");
  const batteryOptions = $("battery-options");
  const batteryEject = $("battery-eject");
  const powerSwitch = $("power-switch");
  const rearPowerSwitch = $("rear-power-switch");
  const powerSwitches = [powerSwitch, rearPowerSwitch];
  const powerLed = document.querySelector(".power-led");
  const cartTray = $("cart-tray");
  const trayError = $("tray-error");
  const batteryTrayError = $("battery-tray-error");
  const deviceMessage = $("device-message");
  const screenTranscript = $("engine-transcript");
  const volumeWheels = [$("volume-wheel"), $("rear-volume-wheel")];
  const volumeMeter = $("volume-meter");
  const speaker = createSpeaker();
  const context = canvas.getContext("2d", { alpha: false });
  context.imageSmoothingEnabled = false;
  const image = context.createImageData(WIDTH, HEIGHT);

  let powered = false;
  let ready = false;
  let cartridge = null;
  let cartridges = [];
  let trayOpen = false;
  let batteryTrayOpen = false;
  let picking = false;
  let inserting = false;
  let ejecting = false;
  let trayReturnFocus = null;
  let framePending = false;
  let audioPending = false;
  let transcriptPending = false;
  let transcriptPolledAt = -Infinity;
  let transcriptSeq = 0;
  let bootTimer = null;
  let bootStartedAt = 0;
  let bootFinishing = false;
  let bootGeneration = 0;
  let bootHeld = false;
  let messageTimer = null;
  let messageSurface = null;
  let shellBackVisible = false;
  let shellTurning = false;
  let batteryDoorOpen = false;
  let batteryChanging = false;
  let installedProvider = null;
  let verifiedProvider = null;
  let powerTransitioning = false;
  let powerGeneration = 0;
  let lastPowerFailure = "";
  let providerCheck = null;
  const held = Object.create(null);
  const swallowedByBoot = Object.create(null);

  function fit() {
    const available = Math.min(
      window.innerWidth / DEVICE_WIDTH,
      window.innerHeight / DEVICE_HEIGHT,
    );
    const snapped = available >= 1 ? Math.floor(available * 2) / 2 : Math.max(0.35, available);
    scaleEl.style.zoom = snapped;
  }

  function setPhysicalPowerSwitch({ on, label }) {
    for (const switchControl of powerSwitches) {
      switchControl.classList.toggle("on", on);
      switchControl.setAttribute("aria-label", label);
    }
  }

  /* The speaker drains engine notes on its own in-flight guard, so a slow
   * audio poll never delays the next framebuffer read, or the reverse. */
  async function pollAudio() {
    if (audioPending) return;
    audioPending = true;
    try {
      speaker.play(await invoke("engine_audio"));
    } catch (error) {
      console.error("CQA: failed to read Bevy audio", error);
    } finally {
      audioPending = false;
    }
  }

  /* The engine describes each screen for screen readers. The shell polls at a
   * modest cadence behind its own in-flight guard and replaces the live
   * region only when the engine publishes a new sequence number, so an
   * unchanged screen is never announced twice. */
  async function pollTranscript() {
    const now = performance.now();
    if (transcriptPending || now - transcriptPolledAt < TRANSCRIPT_POLL_MS) return;
    transcriptPending = true;
    transcriptPolledAt = now;
    try {
      const update = await invoke("engine_transcript", { since: transcriptSeq });
      if (update && Number.isInteger(update.seq) && update.seq !== transcriptSeq) {
        transcriptSeq = update.seq;
        screenTranscript.textContent = update.text;
      }
    } catch (error) {
      console.error("CQA: failed to read Bevy transcript", error);
    } finally {
      transcriptPending = false;
    }
  }

  function renderVolume(level) {
    const label = VOLUME_LABELS[level];
    for (const wheel of volumeWheels) {
      wheel.dataset.level = level;
      wheel.setAttribute("aria-valuenow", String(VOLUME_LEVELS.indexOf(level)));
      wheel.setAttribute("aria-valuetext", label);
      wheel.setAttribute("aria-label", `Volume wheel, ${label}`);
    }
    volumeMeter.dataset.level = level;
  }

  function setVolume(level) {
    renderVolume(speaker.setVolume(level));
  }

  async function drawFrame() {
    void pollAudio();
    void pollTranscript();
    if (!framePending) {
      framePending = true;
      try {
        const raw = await invoke("engine_frame");
        const bytes = raw instanceof ArrayBuffer
          ? new Uint8ClampedArray(raw)
          : new Uint8ClampedArray(raw?.buffer || raw || []);
        if (bytes.byteLength === FRAME_BYTES) {
          image.data.set(bytes);
          context.putImageData(image, 0, 0);
        }
      } catch (error) {
        console.error("CQA: failed to read Bevy frame", error);
      } finally {
        framePending = false;
      }
    }
    window.requestAnimationFrame(drawFrame);
  }

  function clearBootTimer() {
    if (bootTimer !== null) {
      window.clearTimeout(bootTimer);
      bootTimer = null;
    }
  }

  function hideDeviceBoot() {
    clearBootTimer();
    bootGeneration += 1;
    bootFinishing = false;
    bootHeld = false;
    bootOverlay.classList.remove("active");
  }

  async function finishDeviceBoot(generation = bootGeneration) {
    if (bootHeld || !powered || !cartridge || bootFinishing || generation !== bootGeneration) return;
    bootFinishing = true;
    clearBootTimer();
    try {
      await invoke("engine_finish_boot");
      if (powered && generation === bootGeneration) hideDeviceBoot();
    } catch (error) {
      if (generation === bootGeneration) bootFinishing = false;
      showDeviceError(error);
    }
  }

  function showDeviceBoot({ hold = false } = {}) {
    clearBootTimer();
    const generation = ++bootGeneration;
    bootStartedAt = performance.now();
    bootFinishing = false;
    bootHeld = hold;
    bootOverlay.classList.remove("active");
    void bootOverlay.offsetWidth;
    bootOverlay.classList.add("active");
    if (cartridge && !bootHeld) {
      bootTimer = window.setTimeout(() => finishDeviceBoot(generation), BOOT_DURATION_MS);
    }
    return generation;
  }

  function releaseDeviceBoot(generation = bootGeneration) {
    if (!powered || generation !== bootGeneration) return;
    bootHeld = false;
    if (!cartridge) return;
    const elapsed = performance.now() - bootStartedAt;
    const remaining = Math.max(0, BOOT_DURATION_MS - elapsed);
    clearBootTimer();
    if (remaining === 0) void finishDeviceBoot(generation);
    else bootTimer = window.setTimeout(() => finishDeviceBoot(generation), remaining);
  }

  function wait(milliseconds) {
    return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
  }

  function waitForPaint() {
    return new Promise((resolve) => {
      window.requestAnimationFrame(() => window.requestAnimationFrame(resolve));
    });
  }

  function normalizeProvider(value) {
    const provider = String(value || "").trim().toLowerCase();
    return Object.hasOwn(PROVIDERS, provider) ? provider : null;
  }

  function providerLabel(provider = installedProvider) {
    return PROVIDERS[provider]?.label || "AI";
  }

  function setBatteryStatus(message, tone = "idle") {
    batteryStatus.textContent = message;
    batteryStatus.classList.remove("ready", "checking", "failed");
    if (tone !== "idle") batteryStatus.classList.add(tone);
  }

  function renderProviderBatteries() {
    const hasProvider = Boolean(installedProvider);
    batteryCompartment.dataset.provider = installedProvider || "";
    batteryPack.classList.toggle("hidden", !hasProvider);
    batteryChooser.classList.toggle("hidden", hasProvider);
    batteryPack.classList.remove("codex", "claude");
    if (installedProvider) batteryPack.classList.add(installedProvider);
    batteryPack.querySelectorAll(".battery-word").forEach((word) => {
      word.textContent = providerLabel();
    });
    batteryPack.setAttribute(
      "aria-label",
      hasProvider
        ? `${providerLabel()} batteries installed. Press to open the battery tray and eject them.`
        : "No AI provider batteries installed",
    );
    if (!hasProvider) setBatteryStatus("NO BATTERIES");
    else if (lastPowerFailure) setBatteryStatus(`${providerLabel()} · NOT READY`, "failed");
    else if (verifiedProvider === installedProvider) setBatteryStatus(`${providerLabel()} · READY`, "ready");
    else setBatteryStatus(`${providerLabel()} · UNTESTED`);
    if (batteryTrayOpen) renderBatteryTray();
    updateControlGuides();
  }

  function setBatteryDoorOpen(open, { force = false } = {}) {
    const nextOpen = Boolean(open);
    if (nextOpen && powered && !force) {
      showDeviceError("TURN POWER OFF TO CHANGE BATTERIES");
      batteryCompartment.classList.add("locked");
      return false;
    }
    if (!nextOpen && batteryTrayOpen) closeBatteryTray({ restoreFocus: false });
    batteryDoorOpen = nextOpen;
    batteryCompartment.classList.toggle("open", batteryDoorOpen);
    batteryCompartment.classList.toggle("locked", powered);
    batteryDoor.setAttribute("aria-expanded", String(batteryDoorOpen));
    batteryDoor.setAttribute(
      "aria-label",
      batteryDoorOpen
        ? "Close AI provider battery compartment"
        : "Open AI provider battery compartment",
    );
    batteryPack.inert = !batteryDoorOpen;
    batteryChooser.inert = !batteryDoorOpen;
    batteryLidSlot.inert = !batteryDoorOpen;
    updateControlGuides();
    return true;
  }

  function persistInstalledProvider() {
    if (installedProvider) localStorage.setItem(PROVIDER_STORAGE_KEY, installedProvider);
    else localStorage.removeItem(PROVIDER_STORAGE_KEY);
  }

  async function setInstalledProvider(provider) {
    if (powered || powerTransitioning || batteryChanging) {
      showDeviceError("TURN POWER OFF TO CHANGE BATTERIES");
      return false;
    }
    const nextProvider = normalizeProvider(provider);
    if (nextProvider && installedProvider) {
      showDeviceError("EJECT INSTALLED BATTERIES FIRST");
      return false;
    }
    const previousProvider = installedProvider;
    const previousVerified = verifiedProvider;
    batteryChanging = true;
    installedProvider = nextProvider;
    verifiedProvider = null;
    lastPowerFailure = "";
    renderProviderBatteries();
    try {
      await invoke("engine_set_ai_provider", { provider: installedProvider });
      persistInstalledProvider();
      return true;
    } catch (error) {
      installedProvider = previousProvider;
      verifiedProvider = previousVerified;
      renderProviderBatteries();
      showDeviceError(`BATTERY CONTACT FAILED · ${deviceMessageText(error)}`);
      return false;
    } finally {
      batteryChanging = false;
    }
  }

  function checkProvider(provider) {
    // A readiness probe is a real CLI call that cannot be cancelled, so a
    // power-on that follows an abandoned one joins its in-flight probe.
    if (providerCheck?.provider === provider) return providerCheck.result;
    const check = { provider, result: invoke("verify_ai_provider", { provider }) };
    const settle = () => {
      if (providerCheck === check) providerCheck = null;
    };
    providerCheck = check;
    check.result.then(settle, settle);
    return check.result;
  }

  async function verifyInstalledProvider(generation) {
    if (!installedProvider) throw new Error("INSTALL AI BATTERIES");
    const provider = installedProvider;
    const current = () => generation === powerGeneration;
    batteryCompartment.classList.add("checking");
    setBatteryStatus(`${providerLabel()} · CHECKING`, "checking");
    try {
      const result = await checkProvider(provider);
      if (!result?.ready || normalizeProvider(result.provider) !== provider) {
        throw new Error(`${providerLabel(provider)} READINESS CHECK FAILED`);
      }
      if (!current()) return;
      verifiedProvider = provider;
      lastPowerFailure = "";
      setBatteryStatus(`${providerLabel()} · READY`, "ready");
    } catch (error) {
      if (current()) {
        verifiedProvider = null;
        setBatteryStatus(`${providerLabel()} · NOT READY`, "failed");
      }
      throw error;
    } finally {
      if (current()) batteryCompartment.classList.remove("checking");
    }
  }

  function powerFailureReason(error) {
    if (!installedProvider) return "NO BATTERIES";
    return deviceMessageText(error, {
      fallback: `${providerLabel()} NOT READY`,
      maxLength: GUIDE_MESSAGE_MAX_LENGTH,
    });
  }

  async function rejectPowerOn(message) {
    powered = false;
    hideDeviceBoot();
    lastPowerFailure = powerFailureReason(message);
    renderProviderBatteries();
    showDeviceError(lastPowerFailure);
    setPhysicalPowerSwitch({ on: false, label: `Power switch, off: ${lastPowerFailure}` });
    batteryCompartment.classList.remove("locked");
    powerLed.classList.remove("off", "checking");
    powerLed.classList.add("rejected");
    await invoke("engine_power", { powered: false }).catch(() => {});
    await wait(POWER_REJECTION_MS);
    powerLed.classList.remove("rejected");
    powerLed.classList.add("off");
    updateControlGuides();
    if (message) console.warn("CQA: power rejected", message);
  }

  function renderBatteryGuides(failure) {
    viewGuide.querySelector(".guide-action").textContent = failure || "CHECK BATTERIES";
    viewGuide.querySelector(".guide-detail").textContent = failure
      ? "TURN UNIT OVER TO CHECK BATTERIES"
      : "TURN UNIT OVER";
    viewGuide.setAttribute(
      "aria-label",
      failure
        ? `${failure}. Turn the device over to check the batteries`
        : "Turn the device over to check the batteries",
    );
    batteryGuide.querySelector(".guide-action").textContent = failure || "OPEN BATTERY TAB";
    batteryGuide.querySelector(".guide-detail").textContent = failure
      ? "FIX IT AND POWER ON · OR OPEN TAB TO SWAP"
      : "TO LOAD 2×AA";
    batteryGuide.setAttribute(
      "aria-label",
      failure
        ? `${failure}. Fix it and power on again, or open the battery cover to swap batteries`
        : "Open the battery cover to load batteries",
    );
  }

  function updateControlGuides() {
    const needsBatteryCheck =
      ready &&
      !shellBackVisible &&
      !trayOpen &&
      !batteryTrayOpen &&
      !powerTransitioning &&
      !powered &&
      (!installedProvider || Boolean(lastPowerFailure));
    const needsCart =
      ready &&
      !shellBackVisible &&
      !trayOpen &&
      !batteryTrayOpen &&
      !cartridge &&
      !powered &&
      !needsBatteryCheck &&
      Boolean(installedProvider);
    const needsPower =
      ready &&
      !shellBackVisible &&
      !trayOpen &&
      !batteryTrayOpen &&
      !powerTransitioning &&
      !needsBatteryCheck &&
      Boolean(cartridge) !== powered;
    const batteryFailure = installedProvider ? lastPowerFailure : "";
    const needsBatteryTab =
      ready &&
      shellBackVisible &&
      !batteryTrayOpen &&
      !powered &&
      (!installedProvider || Boolean(batteryFailure)) &&
      !batteryDoorOpen;
    const needsBatteryBay =
      ready &&
      shellBackVisible &&
      !batteryTrayOpen &&
      !powered &&
      (!installedProvider || Boolean(batteryFailure)) &&
      batteryDoorOpen;
    cartGuide.classList.toggle("hidden", !needsCart);
    powerGuide.classList.toggle("hidden", !needsPower);
    viewGuide.classList.toggle("hidden", !needsBatteryCheck);
    batteryGuide.classList.toggle("hidden", !needsBatteryTab);
    $("cart-back").classList.toggle("guided", needsCart);
    $("power-switch").classList.toggle("guided", needsPower);
    batteryDoor.classList.toggle("guided", needsBatteryTab);
    batteryBay.classList.toggle("guided", needsBatteryBay);
    renderBatteryGuides(batteryFailure);
    const switchingOff = powered && !cartridge;
    powerGuide.classList.toggle("switching-off", switchingOff);
    powerGuide.querySelector(".guide-action").textContent = switchingOff
      ? "TURN POWER OFF"
      : "TURN POWER ON";
    powerGuide.querySelector(".guide-detail").textContent = switchingOff
      ? "TO LOAD A GAME"
      : "TO START";
    powerGuide.setAttribute(
      "aria-label",
      switchingOff
        ? "Turn the power off to load a game"
        : "Turn the power on to start",
    );
  }

  function setShellBackVisible(visible) {
    shellBackVisible = Boolean(visible);
    scaleEl.classList.toggle("showing-back", shellBackVisible);
    frontFace.setAttribute("aria-hidden", String(shellBackVisible));
    backFace.setAttribute("aria-hidden", String(!shellBackVisible));
    frontFace.inert = shellBackVisible;
    backFace.inert = !shellBackVisible;
    viewToggle.classList.toggle("back-active", shellBackVisible);
    viewToggle.setAttribute("aria-checked", String(shellBackVisible));
    viewToggle.setAttribute("aria-label", shellBackVisible ? "Show front of device" : "Show back of device");
    updateControlGuides();
  }

  function turnShell() {
    if (shellTurning) return;
    const nextBackVisible = !shellBackVisible;
    const swapFaces = () => {
      setShellBackVisible(nextBackVisible);
    };

    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      swapFaces();
      return;
    }

    shellTurning = true;
    const directionClass = nextBackVisible ? "turning-to-back" : "turning-to-front";
    scaleEl.classList.add("turning", directionClass);
    window.setTimeout(swapFaces, TURN_DURATION_MS / 2);
    window.setTimeout(() => {
      scaleEl.classList.remove("turning", directionClass);
      shellTurning = false;
    }, TURN_DURATION_MS);
  }

  async function setPower(on) {
    const target = Boolean(on);
    // initialize() restores the batteries and the saved cartridge and then
    // switches the engine off, so power input waits until it has finished.
    if (!ready || powered === target || (target && powerTransitioning)) return;
    if (target && cartridgeBusy()) {
      showDeviceError("WAIT FOR THE CARTRIDGE");
      return;
    }
    const generation = ++powerGeneration;
    powerTransitioning = true;

    if (target) {
      if (trayOpen) closeTray();
      if (batteryTrayOpen) closeBatteryTray({ restoreFocus: false });
      powered = true;
      lastPowerFailure = "";
      renderProviderBatteries();
      setPhysicalPowerSwitch({ on: true, label: "Power switch, checking AI batteries" });
      powerLed.classList.remove("off", "rejected");
      powerLed.classList.add("checking");
      batteryCompartment.classList.add("locked");
      const providerBootGeneration = showDeviceBoot({ hold: true });
      updateControlGuides();
      try {
        await waitForPaint();
        if (generation !== powerGeneration || !powered) return;
        await verifyInstalledProvider(generation);
        if (generation !== powerGeneration || !powered) return;
        await invoke("engine_power", { powered: true });
        if (generation !== powerGeneration || !powered) {
          await invoke("engine_power", { powered: false }).catch(() => {});
          return;
        }
        setPhysicalPowerSwitch({ on: true, label: `Power on, ${providerLabel()} batteries ready` });
        powerLed.classList.remove("checking");
        releaseDeviceBoot(providerBootGeneration);
      } catch (error) {
        if (generation === powerGeneration && powered) await rejectPowerOn(error);
      } finally {
        if (generation === powerGeneration) {
          powerTransitioning = false;
          updateControlGuides();
        }
      }
      return;
    }

    powered = false;
    setPhysicalPowerSwitch({ on: false, label: "Power switch, off" });
    powerLed.classList.remove("checking", "rejected");
    powerLed.classList.add("off");
    batteryCompartment.classList.remove("locked", "checking");
    hideDeviceBoot();
    renderProviderBatteries();
    try {
      await invoke("engine_power", { powered: false });
    } catch (error) {
      if (generation === powerGeneration) {
        powered = true;
        setPhysicalPowerSwitch({ on: true, label: "Power on; shutdown failed" });
        powerLed.classList.remove("off");
        batteryCompartment.classList.add("locked");
        showDeviceError(`SHUTDOWN FAILED · ${deviceMessageText(error)}`);
      }
    } finally {
      if (generation === powerGeneration) {
        powerTransitioning = false;
        updateControlGuides();
      }
    }
  }

  function renderCartridge() {
    const slot = $("cart-back");
    const rearSlot = $("rear-cart-back");
    if (cartridge) {
      slot.className = "loaded";
      slot.style.setProperty("--cart-color", cartridge.color || "#6a6fd1");
      slot.title = `CARTRIDGE: ${cartridge.title}`;
      rearSlot.className = "loaded";
      rearSlot.style.setProperty("--cart-color", cartridge.color || "#6a6fd1");
      rearSlot.title = `CARTRIDGE: ${cartridge.title}`;
    } else {
      slot.className = "empty";
      slot.style.removeProperty("--cart-color");
      slot.title = "CARTRIDGE SLOT (EMPTY)";
      rearSlot.className = "empty";
      rearSlot.style.removeProperty("--cart-color");
      rearSlot.title = "CARTRIDGE SLOT (EMPTY)";
    }
    updateControlGuides();
  }

  function persistCartridges() {
    cartridges = normalizeCartridges(cartridges);
    const metadata = cartridges.map(({ path, title, branch, revision, color }) => ({
      path,
      title,
      branch,
      revision,
      color,
    }));
    localStorage.setItem("cqa-repo-carts", JSON.stringify(metadata));
  }

  function cacheCartridge(value) {
    const result = upsertCartridge(cartridges, value);
    cartridges = result.items;
    if (!result.accepted) return false;
    persistCartridges();
    return true;
  }

  function forgetCartridge(path) {
    cartridges = cartridges.filter((entry) => entry.path !== path);
    // A saved slot that failed to load at startup is kept for a retry; once
    // its rack entry goes, the next launch must not bring it back.
    if (localStorage.getItem("cqa-cart-id") === path) localStorage.removeItem("cqa-cart-id");
    persistCartridges();
  }

  async function refreshCartridgeBranches() {
    const updates = await Promise.all(cartridges.map(async ({ path }) => {
      try {
        return { path, branch: await invoke("cartridge_branch", { path }) };
      } catch (_) {
        return null;
      }
    }));
    const changedPaths = [];
    for (const update of updates) {
      if (!update) continue;
      const index = cartridges.findIndex(({ path }) => path === update.path);
      if (index < 0 || cartridges[index].branch === update.branch) continue;
      cartridges[index] = { ...cartridges[index], branch: update.branch };
      changedPaths.push(update.path);
    }
    if (!changedPaths.length) return;
    persistCartridges();
    if (!trayOpen) return;
    for (const value of cartridges) {
      if (changedPaths.includes(value.path)) relabelTrayCard(value);
    }
  }

  function activeMessageSurface() {
    if (trayOpen) return trayError;
    if (batteryTrayOpen) return batteryTrayError;
    return deviceMessage;
  }

  function hideDeviceMessage() {
    if (messageTimer !== null) window.clearTimeout(messageTimer);
    messageTimer = null;
    if (messageSurface) messageSurface.textContent = "";
    messageSurface = null;
  }

  function showDeviceMessage(message, tone = "error", { persist = false } = {}) {
    hideDeviceMessage();
    messageSurface = activeMessageSurface();
    messageSurface.classList.toggle("notice", tone === "notice");
    messageSurface.textContent = deviceMessageText(message);
    if (!persist) messageTimer = window.setTimeout(hideDeviceMessage, MESSAGE_DURATION_MS);
  }

  function showDeviceError(message, options) {
    showDeviceMessage(message, "error", options);
  }

  /* Loading or ejecting a cartridge is an async engine call that can walk git
   * history for seconds. Power and the slot never change under each other:
   * power-on waits for the cartridge, and the slot refuses while power is on
   * or switching, so the engine and the shell always agree on the game. */
  function cartridgeBusy() {
    return picking || inserting || ejecting;
  }

  async function insertCartridge(value) {
    if (!value || cartridge || inserting) return;
    if (powered || powerTransitioning) {
      showDeviceError("TURN POWER OFF TO LOAD A GAME");
      return;
    }
    const alreadyCached = cartridges.some((entry) => entry.path === value.path);
    if (!alreadyCached && cartridges.length >= MAX_CARTRIDGES) {
      showDeviceError("CARTRIDGE RACK FULL · RECYCLE ONE FIRST");
      return;
    }
    inserting = true;
    try {
      const configured = await invoke("engine_set_cartridge", { path: value.path });
      cartridge = configured;
      localStorage.setItem("cqa-cart-id", configured.path);
      if (!cacheCartridge(configured)) {
        await invoke("engine_set_cartridge", { path: null });
        cartridge = null;
        localStorage.removeItem("cqa-cart-id");
        showDeviceError("CARTRIDGE RACK FULL · RECYCLE ONE FIRST");
        return;
      }
    } finally {
      inserting = false;
    }
    renderCartridge();
    closeTray();
  }

  async function insertByPath(path) {
    if (cartridge) return;
    try {
      await insertCartridge({ path });
    } catch (error) {
      // Only a folder that is no longer a repository leaves the rack on its
      // own; fixable faults (bad CODEQUEST.toml, missing drive) stay racked.
      if (isRefusedCartridgeError(error)) {
        forgetCartridge(path);
        if (trayOpen) buildTray();
      }
      showDeviceError(error);
    }
  }

  async function addFromDisk() {
    if (picking || cartridge) return;
    if (cartridges.length >= MAX_CARTRIDGES) {
      showDeviceError("CARTRIDGE RACK FULL · RECYCLE ONE FIRST");
      return;
    }
    picking = true;
    try {
      await insertCartridge(await invoke("pick_cartridge"));
    } catch (error) {
      showDeviceError(error);
    } finally {
      picking = false;
    }
  }

  function ejectCartridge() {
    if (!cartridge || ejecting) return;
    if (powered || powerTransitioning) {
      showDeviceError("TURN POWER OFF TO EJECT");
      return;
    }
    ejecting = true;
    const slot = $("cart-back");
    slot.classList.add("ejecting");
    window.setTimeout(async () => {
      try {
        await invoke("engine_set_cartridge", { path: null });
        cartridge = null;
        localStorage.removeItem("cqa-cart-id");
        renderCartridge();
        if (trayOpen) buildTray();
      } catch (error) {
        slot.classList.remove("ejecting");
        showDeviceError(error);
      } finally {
        ejecting = false;
      }
    }, 240);
  }

  function escapeHtml(value) {
    return String(value).replace(/[<>&"]/g, (char) => ({
      "<": "&lt;", ">": "&gt;", "&": "&amp;", "\"": "&quot;",
    })[char]);
  }

  function recycleCartridge(value, card) {
    if (card.classList.contains("recycling")) return;
    if (cartridge?.path === value.path) {
      showDeviceError("EJECT THIS CARTRIDGE BEFORE RECYCLING IT");
      return;
    }
    const focusIndex = [...card.parentElement.children].indexOf(card);
    card.classList.add("recycling");
    card.disabled = true;
    window.setTimeout(() => {
      forgetCartridge(value.path);
      if (trayOpen) buildTray({ focusIndex });
      showDeviceMessage(`RECYCLED ${value.title} · REPOSITORY UNTOUCHED`, "notice");
    }, 180);
  }

  function bindCartridgeDrag(card, value, current) {
    let pointerId = null;
    let startY = 0;
    let deltaY = 0;
    let moved = false;

    const clearDrag = () => {
      pointerId = null;
      card.classList.remove("dragging", "load-ready", "recycle-ready");
      card.style.removeProperty("--drag-y");
    };

    card.addEventListener("pointerdown", (event) => {
      if (event.button !== 0 || pointerId !== null) return;
      event.preventDefault();
      event.stopPropagation();
      card.focus();
      pointerId = event.pointerId;
      startY = event.clientY;
      deltaY = 0;
      moved = false;
      card.setPointerCapture(event.pointerId);
      card.classList.add("dragging");
    });

    card.addEventListener("pointermove", (event) => {
      if (event.pointerId !== pointerId) return;
      deltaY = Math.max(-72, Math.min(72, event.clientY - startY));
      moved ||= Math.abs(deltaY) > 6;
      card.style.setProperty("--drag-y", `${deltaY}px`);
      const intent = cartridgeDragIntent(deltaY, {
        canLoad: !cartridge,
        canRecycle: !current,
      });
      card.classList.toggle("load-ready", intent === "load");
      card.classList.toggle("recycle-ready", intent === "recycle");
    });

    card.addEventListener("pointerup", (event) => {
      if (event.pointerId !== pointerId) return;
      event.preventDefault();
      event.stopPropagation();
      const intent = cartridgeDragIntent(deltaY, {
        canLoad: !cartridge,
        canRecycle: !current,
      });
      const deniedLoad = deltaY <= -CARTRIDGE_DRAG_THRESHOLD && Boolean(cartridge);
      const deniedRecycle = deltaY >= CARTRIDGE_DRAG_THRESHOLD && current;
      clearDrag();
      if (intent === "load") insertByPath(value.path);
      else if (intent === "recycle") recycleCartridge(value, card);
      else if (deniedLoad) showDeviceError("EJECT THE CURRENT CARTRIDGE BEFORE LOADING ANOTHER");
      else if (deniedRecycle) showDeviceError("EJECT THIS CARTRIDGE BEFORE RECYCLING IT");
      else if (!moved && !cartridge) insertByPath(value.path);
    });

    card.addEventListener("pointercancel", clearDrag);
    card.addEventListener("keydown", (event) => {
      if (event.repeat) return;
      if ((event.key === "Enter" || event.key === " ") && !cartridge) {
        event.preventDefault();
        insertByPath(value.path);
      } else if ((event.key === "Delete" || event.key === "Backspace") && !current) {
        event.preventDefault();
        recycleCartridge(value, card);
      }
    });
  }

  function cartridgeCardLabel(value, current) {
    const accessibilityAction = current
      ? "Currently in the device. Use the Eject Cartridge control before recycling."
      : cartridge
        ? "Drag down or press Delete to recycle. Eject the current cartridge before loading."
        : "Drag up or press Enter to load. Drag down or press Delete to recycle.";
    return `${value.title}, branch ${value.branch}. ${accessibilityAction}`;
  }

  function relabelTrayCard(value) {
    const card = [...$("tray-carts").children].find((element) => element.dataset.path === value.path);
    if (!card) return;
    card.querySelector(".cc-sub").textContent = value.branch;
    card.setAttribute("aria-label", cartridgeCardLabel(value, cartridge?.path === value.path));
  }

  function buildTray({ focusIndex = null } = {}) {
    const list = $("tray-carts");
    const focused = list.contains(document.activeElement) ? document.activeElement : null;
    const focusPath = focused?.dataset.path ?? null;
    const restoreIndex = focused ? [...list.children].indexOf(focused) : focusIndex;
    list.innerHTML = "";
    for (const value of cartridges) {
      const card = document.createElement("button");
      const current = cartridge?.path === value.path;
      card.type = "button";
      card.className = `cart-card${current ? " current" : ""}`;
      card.dataset.path = value.path;
      card.setAttribute("aria-label", cartridgeCardLabel(value, current));
      const gesture = current ? "EJECT FIRST" : "↑ LOAD · ↓ RECYCLE";
      card.innerHTML = `<span class="cc-strip">CODEQUEST ADVANCE</span><span class="cc-label" style="--cc:${escapeHtml(value.color || "#6a6fd1")}"><span class="cc-title">${escapeHtml(value.title)}</span><span class="cc-sub">${escapeHtml(value.branch)}</span><span class="cc-gesture">${gesture}</span></span>`;
      bindCartridgeDrag(card, value, current);
      list.appendChild(card);
    }

    if (!cartridge && cartridges.length < MAX_CARTRIDGES) {
      const add = document.createElement("button");
      add.type = "button";
      add.className = "cart-card add";
      add.innerHTML = `<span class="cc-label"><span class="cc-title">+ ADD FROM DISK</span><span class="cc-sub">CARTRIDGE ${cartridges.length + 1} OF ${MAX_CARTRIDGES}</span></span>`;
      add.addEventListener("click", (event) => {
        event.stopPropagation();
        addFromDisk();
      });
      list.appendChild(add);
    }

    if (cartridge) {
      const eject = document.createElement("button");
      eject.type = "button";
      eject.className = "cart-card eject";
      eject.innerHTML = `<span class="cc-label"><span class="cc-title">EJECT CARTRIDGE</span><span class="cc-sub">RETURN IT TO THE RACK</span></span>`;
      eject.addEventListener("click", (event) => {
        event.stopPropagation();
        ejectCartridge();
      });
      list.appendChild(eject);
    }
    $("cart-tray-head").textContent = `CARTRIDGE RACK · ${cartridges.length}/${MAX_CARTRIDGES}`;
    document.querySelector(".tray-hint").textContent = cartridge
      ? "EJECT CURRENT · DRAG OTHER CARTS DOWN TO RECYCLE"
      : "DRAG ↑ TO LOAD · DRAG ↓ TO RECYCLE · ESC TO CLOSE";
    const cards = [...list.children];
    const restore = rackFocusIndex(cards.map((card) => card.dataset.path), focusPath, restoreIndex);
    if (restore >= 0) cards[restore].focus();
  }

  function syncTrayModality() {
    const modal = trayOpen || batteryTrayOpen;
    scaleEl.inert = modal;
    viewToggle.inert = modal;
    viewGuide.inert = modal;
  }

  function trapTabFocus(event, container) {
    const focusables = [...container.querySelectorAll("button:not(:disabled)")];
    const target = trappedFocusTarget(focusables, document.activeElement, event.shiftKey);
    if (target || !focusables.length) event.preventDefault();
    target?.focus();
  }

  function openTray() {
    if (!ready || powered || powerTransitioning) return;
    if (batteryTrayOpen) closeBatteryTray({ restoreFocus: false });
    const active = document.activeElement;
    trayReturnFocus = active instanceof HTMLElement && active !== document.body ? active : null;
    buildTray();
    cartTray.classList.remove("hidden");
    cartTray.setAttribute("aria-hidden", "false");
    trayOpen = true;
    syncTrayModality();
    $("tray-carts").querySelector("button:not(:disabled)")?.focus();
    refreshCartridgeBranches().catch(() => {});
    updateControlGuides();
  }

  function closeTray() {
    const focusWasInTray =
      cartTray.contains(document.activeElement) || document.activeElement === document.body;
    cartTray.classList.add("hidden");
    cartTray.setAttribute("aria-hidden", "true");
    trayOpen = false;
    if (messageSurface === trayError) hideDeviceMessage();
    syncTrayModality();
    if (focusWasInTray) trayReturnFocus?.focus();
    trayReturnFocus = null;
    updateControlGuides();
  }

  function renderBatteryTray() {
    const hasProvider = Boolean(installedProvider);
    batteryOptions.querySelectorAll("[data-provider]").forEach((choice) => {
      const current = choice.dataset.provider === installedProvider;
      choice.classList.toggle("current", current);
      choice.setAttribute("aria-pressed", String(current));
      choice.disabled = hasProvider;
    });
    batteryEject.disabled = !hasProvider;
    document.querySelector(".battery-tray-hint").textContent = !hasProvider
      ? "SELECT A PACK · ESC TO CLOSE"
      : lastPowerFailure
        ? `${lastPowerFailure} · EJECT TO SWAP`
        : "EJECT CURRENT PACK BEFORE LOADING ANOTHER";
  }

  function focusBatteryTray() {
    const firstAction = installedProvider
      ? batteryEject
      : batteryOptions.querySelector("[data-provider]");
    firstAction?.focus();
  }

  function openBatteryTray() {
    if (!ready || powered || batteryChanging || !batteryDoorOpen) return;
    if (trayOpen) closeTray();
    renderBatteryTray();
    batteryTray.classList.remove("hidden");
    batteryTray.setAttribute("aria-hidden", "false");
    batteryTrayOpen = true;
    syncTrayModality();
    focusBatteryTray();
    updateControlGuides();
  }

  function closeBatteryTray({ restoreFocus = true } = {}) {
    batteryTray.classList.add("hidden");
    batteryTray.setAttribute("aria-hidden", "true");
    batteryTrayOpen = false;
    if (messageSurface === batteryTrayError) hideDeviceMessage();
    syncTrayModality();
    if (restoreFocus && shellBackVisible && batteryDoorOpen) {
      (installedProvider ? batteryPack : batteryChooser).focus();
    }
    updateControlGuides();
  }

  function sendButton(button, pressed) {
    document.querySelectorAll(`[data-btn="${button}"]`).forEach((element) => {
      element.classList.toggle("pressed", pressed);
    });
    if (bootOverlay.classList.contains("active")) {
      if (pressed) {
        swallowedByBoot[button] = true;
        if (cartridge && performance.now() - bootStartedAt >= BOOT_SKIP_DELAY_MS) {
          finishDeviceBoot();
        }
      } else {
        delete swallowedByBoot[button];
      }
      return;
    }
    if (!pressed && swallowedByBoot[button]) {
      delete swallowedByBoot[button];
      return;
    }
    invoke("engine_input", { button, pressed }).catch((error) => {
      console.error("CQA: input rejected", error);
    });
  }

  function isShellControl(target) {
    return target instanceof Element
      && target !== document.body
      && target.matches("button, [role=button], [role=switch]");
  }

  const keyMap = {
    ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right",
    KeyD: "a", KeyS: "b", Enter: "start", NumpadEnter: "start",
    ShiftLeft: "select", ShiftRight: "select", KeyA: "l", KeyF: "r",
  };

  window.addEventListener("keydown", (event) => {
    if (batteryTrayOpen) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeBatteryTray();
      } else if (event.key === "Tab") {
        trapTabFocus(event, batteryOptions);
      }
      return;
    }
    if (event.code === "F1") {
      event.preventDefault();
      if (!event.repeat) turnShell();
      return;
    }
    if (event.code === "KeyP") {
      if (!event.repeat) setPower(!powered);
      return;
    }
    if (event.code === "KeyC") {
      if (!event.repeat) trayOpen ? closeTray() : openTray();
      return;
    }
    if (event.code === "KeyV") {
      if (!event.repeat) setVolume(nextVolume(speaker.volume));
      return;
    }
    if (trayOpen && (event.key === "Escape" || event.code === "KeyS")) {
      closeTray();
      return;
    }
    if (trayOpen && event.key === "Tab") {
      trapTabFocus(event, cartTray);
      return;
    }
    if (leavesKeyToFocusedControl(event.code, isShellControl(event.target))) return;
    const button = keyMap[event.code];
    if (!button) return;
    event.preventDefault();
    if (event.repeat || held[button]) return;
    held[button] = true;
    sendButton(button, true);
  });

  window.addEventListener("keyup", (event) => {
    const button = keyMap[event.code];
    if (!button || !held[button]) return;
    held[button] = false;
    sendButton(button, false);
  });

  window.addEventListener("blur", () => {
    for (const button of Object.keys(held)) {
      if (held[button]) {
        held[button] = false;
        sendButton(button, false);
      }
    }
  });

  document.querySelectorAll("[data-btn]").forEach((element) => {
    const button = element.getAttribute("data-btn");
    const press = (event) => {
      event.preventDefault();
      if (held[button]) return;
      held[button] = true;
      sendButton(button, true);
    };
    const release = () => {
      if (!held[button]) return;
      held[button] = false;
      sendButton(button, false);
    };
    element.addEventListener("pointerdown", press);
    element.addEventListener("pointerup", release);
    element.addEventListener("pointerleave", release);
    element.addEventListener("pointercancel", release);
  });

  // Keep mouse focus off the floating toggle so Enter stays START afterwards;
  // keyboard focus still activates it (see leavesKeyToFocusedControl).
  viewToggle.addEventListener("pointerdown", (event) => event.preventDefault());
  viewToggle.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    turnShell();
  });
  viewGuide.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    turnShell();
  });

  batteryDoor.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    setBatteryDoorOpen(!batteryDoorOpen);
  });
  batteryLidSlot.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    setBatteryDoorOpen(false);
  });
  batteryPack.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    openBatteryTray();
  });
  batteryChooser.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    openBatteryTray();
  });
  batteryOptions.querySelectorAll("[data-provider]").forEach((choice) => {
    choice.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (installedProvider) return;
      setInstalledProvider(choice.dataset.provider).then((changed) => {
        if (changed) closeBatteryTray();
        else if (batteryTrayOpen) focusBatteryTray();
      });
    });
  });
  batteryEject.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    if (!installedProvider) return;
    setInstalledProvider(null).then((changed) => {
      if (changed) batteryOptions.querySelector("[data-provider]")?.focus();
      else if (batteryTrayOpen) focusBatteryTray();
    });
  });

  batteryGuide.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    setBatteryDoorOpen(true);
  });

  /* Browsers only start audio from a user gesture. The first key press, the
   * power switch, or any other press on the device wakes the speaker. */
  for (const gesture of ["keydown", "pointerdown"]) {
    window.addEventListener(gesture, () => speaker.unlock(), { capture: true });
  }

  for (const wheel of volumeWheels) {
    const roll = createWheelDetents();
    wheel.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      event.stopPropagation();
      setVolume(nextVolume(speaker.volume));
    });
    wheel.addEventListener("wheel", (event) => {
      event.preventDefault();
      const detents = roll(event.deltaY, event.deltaMode, performance.now());
      if (!detents) return;
      let level = speaker.volume;
      for (let turned = 0; turned < Math.abs(detents); turned += 1) level = stepVolume(level, detents);
      setVolume(level);
    }, { passive: false });
    wheel.addEventListener("keydown", (event) => {
      const direction = { ArrowUp: 1, ArrowRight: 1, ArrowDown: -1, ArrowLeft: -1 }[event.key];
      if (direction) setVolume(stepVolume(speaker.volume, direction));
      else if (event.key === "Home") setVolume(VOLUME_LEVELS[0]);
      else if (event.key === "End") setVolume(VOLUME_LEVELS.at(-1));
      else if (event.key === "Enter" || event.key === " ") setVolume(nextVolume(speaker.volume));
      else return;
      // A focused wheel owns its keys; they must not also reach the D-pad.
      event.preventDefault();
      event.stopPropagation();
    });
  }

  for (const switchControl of powerSwitches) {
    switchControl.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      event.stopPropagation();
      setPower(!powered);
    });
    switchControl.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      if (event.repeat) return;
      setPower(!powered);
    });
  }
  $("cart-back").addEventListener("pointerdown", (event) => {
    event.preventDefault();
    event.stopPropagation();
    trayOpen ? closeTray() : openTray();
  });
  cartGuide.addEventListener("click", (event) => {
    event.stopPropagation();
    openTray();
  });
  powerGuide.addEventListener("click", (event) => {
    event.stopPropagation();
    setPower(!powered);
  });
  $("cart-tray").addEventListener("pointerdown", (event) => {
    if (event.target === $("cart-tray")) closeTray();
  });
  batteryTray.addEventListener("pointerdown", (event) => {
    if (event.target === batteryTray) closeBatteryTray();
  });
  window.addEventListener("resize", fit);

  // The engine freezes decorative framebuffer motion when the host asks for
  // reduced motion; the shell only reports the preference.
  function syncReducedMotion() {
    const reduced = Boolean(window.matchMedia?.("(prefers-reduced-motion: reduce)").matches);
    invoke("engine_set_reduced_motion", { reduced }).catch((error) => {
      console.error("CQA: failed to forward reduced-motion preference", error);
    });
  }

  async function initialize() {
    fit();
    renderVolume(speaker.volume);
    setShellBackVisible(false);
    syncReducedMotion();
    window.matchMedia?.("(prefers-reduced-motion: reduce)")
      .addEventListener?.("change", syncReducedMotion);
    rearSerial.textContent = await invoke("app_revision");
    installedProvider = normalizeProvider(localStorage.getItem(PROVIDER_STORAGE_KEY));
    if (!installedProvider) localStorage.removeItem(PROVIDER_STORAGE_KEY);
    verifiedProvider = null;
    renderProviderBatteries();
    setBatteryDoorOpen(false, { force: true });
    try {
      await invoke("engine_set_ai_provider", { provider: installedProvider });
    } catch (error) {
      // Keep the saved choice so the next launch retries the same batteries.
      installedProvider = null;
      renderProviderBatteries();
      showDeviceError(`BATTERY CONTACT FAILED · ${deviceMessageText(error)}`);
    }
    const savedPath = localStorage.getItem("cqa-cart-id");
    try {
      const stored = JSON.parse(localStorage.getItem("cqa-repo-carts")) || [];
      cartridges = normalizeCartridges(stored, savedPath);
    } catch (_) {
      cartridges = [];
    }
    persistCartridges();
    if (savedPath) {
      try {
        cartridge = await invoke("engine_set_cartridge", { path: savedPath });
        if (
          !cartridges.some((entry) => entry.path === cartridge.path)
          && cartridges.length >= MAX_CARTRIDGES
        ) {
          cartridges.pop();
        }
        cacheCartridge(cartridge);
      } catch (error) {
        cartridge = null;
        // Only a refused folder is forgotten. A fault that may clear (git
        // unavailable or timed out, a bad CODEQUEST.toml, a missing drive)
        // keeps its rack entry and the saved slot, so the next launch retries.
        if (isRefusedCartridgeError(error)) forgetCartridge(savedPath);
        showDeviceError(`CARTRIDGE NOT LOADED · ${deviceMessageText(error)}`);
      }
    }
    renderCartridge();
    await invoke("engine_power", { powered: false });
    ready = true;
    updateControlGuides();
    window.requestAnimationFrame(drawFrame);
  }

  initialize().catch((error) => {
    showDeviceError(`DEVICE FAULT · ${deviceMessageText(error)}`, { persist: true });
  });

  function createBrowserDemo() {
    const frame = new Uint8Array(FRAME_BYTES);
    for (let offset = 0; offset < frame.length; offset += 4) {
      frame.set([26, 28, 44, 255], offset);
    }
    return async (command, args) => {
      if (command === "engine_frame") return frame.buffer;
      if (command === "engine_audio") return { tick: 0, notes: [] };
      if (command === "engine_transcript") return null;
      if (command === "app_revision") return "0000000";
      if (["engine_power", "engine_finish_boot", "engine_input", "engine_set_ai_provider", "engine_set_reduced_motion"].includes(command)) return null;
      if (command === "verify_ai_provider") return { provider: args?.provider, ready: true };
      if (command === "engine_set_cartridge" && args?.path == null) return null;
      if (command === "engine_set_cartridge") throw new Error("RUN IN TAURI TO LOAD CARTRIDGES");
      if (command === "pick_cartridge") throw new Error("RUN IN TAURI TO LOAD CARTRIDGES");
      if (command === "cartridge_branch") return "BRANCH UNKNOWN";
      throw new Error(`UNKNOWN COMMAND ${command}`);
    };
  }
})();
