/* CODE QUEST ADVANCE device adapter.
 * The browser owns only the shell. Bevy owns game state, timing, process
 * execution, and the fixed 240x160 RGBA framebuffer exposed by Rust. */
import {
  CARTRIDGE_DRAG_THRESHOLD,
  MAX_CARTRIDGES,
  cartridgeDragIntent,
  normalizeCartridges,
  upsertCartridge,
} from "./cartridge-library.js";

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
  const powerLed = document.querySelector(".power-led");
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
  let framePending = false;
  let bootTimer = null;
  let bootStartedAt = 0;
  let bootFinishing = false;
  let bootGeneration = 0;
  let trayMessageTimer = null;
  let shellBackVisible = false;
  let shellTurning = false;
  let batteryDoorOpen = false;
  let batteryChanging = false;
  let installedProvider = null;
  let verifiedProvider = null;
  let powerTransitioning = false;
  let lastPowerFailure = "";
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

  async function drawFrame() {
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
    bootOverlay.classList.remove("active");
  }

  async function finishDeviceBoot(generation = bootGeneration) {
    if (!powered || !cartridge || bootFinishing || generation !== bootGeneration) return;
    bootFinishing = true;
    clearBootTimer();
    try {
      await invoke("engine_finish_boot");
      if (powered && generation === bootGeneration) hideDeviceBoot();
    } catch (error) {
      if (generation === bootGeneration) bootFinishing = false;
      showTrayError(error);
    }
  }

  function showDeviceBoot() {
    clearBootTimer();
    const generation = ++bootGeneration;
    bootStartedAt = performance.now();
    bootFinishing = false;
    bootOverlay.classList.remove("active");
    void bootOverlay.offsetWidth;
    bootOverlay.classList.add("active");
    if (cartridge) {
      bootTimer = window.setTimeout(() => finishDeviceBoot(generation), BOOT_DURATION_MS);
    }
  }

  function wait(milliseconds) {
    return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
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
    else if (verifiedProvider === installedProvider) setBatteryStatus(`${providerLabel()} · READY`, "ready");
    else setBatteryStatus(`${providerLabel()} · UNTESTED`);
    if (batteryTrayOpen) renderBatteryTray();
    updateControlGuides();
  }

  function setBatteryDoorOpen(open, { force = false } = {}) {
    const nextOpen = Boolean(open);
    if (nextOpen && powered && !force) {
      setBatteryStatus("TURN POWER OFF TO CHANGE BATTERIES", "failed");
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
      setBatteryStatus("TURN POWER OFF TO CHANGE BATTERIES", "failed");
      return false;
    }
    const nextProvider = normalizeProvider(provider);
    if (nextProvider && installedProvider) {
      setBatteryStatus("EJECT INSTALLED BATTERIES FIRST", "failed");
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
      setBatteryStatus("BATTERY CONTACT FAILED", "failed");
      showTrayError(error);
      return false;
    } finally {
      batteryChanging = false;
    }
  }

  async function verifyInstalledProvider() {
    if (!installedProvider) throw new Error("INSTALL AI BATTERIES");
    if (verifiedProvider === installedProvider) return;
    batteryCompartment.classList.add("checking");
    setBatteryStatus(`${providerLabel()} · CHECKING`, "checking");
    try {
      const result = await invoke("verify_ai_provider", { provider: installedProvider });
      if (!result?.ready || normalizeProvider(result.provider) !== installedProvider) {
        throw new Error(`${providerLabel()} READINESS CHECK FAILED`);
      }
      verifiedProvider = installedProvider;
      lastPowerFailure = "";
      setBatteryStatus(`${providerLabel()} · READY`, "ready");
    } catch (error) {
      verifiedProvider = null;
      setBatteryStatus(`${providerLabel()} · NOT READY`, "failed");
      throw error;
    } finally {
      batteryCompartment.classList.remove("checking");
    }
  }

  async function rejectPowerOn(message) {
    powered = false;
    hideDeviceBoot();
    lastPowerFailure = installedProvider ? `${providerLabel()} NOT READY` : "NO BATTERIES";
    powerSwitch.classList.remove("checking");
    powerSwitch.classList.add("on");
    powerLed.classList.remove("off", "checking");
    powerLed.classList.add("rejected");
    powerSwitch.setAttribute("aria-label", `Power rejected: ${lastPowerFailure}`);
    await invoke("engine_power", { powered: false }).catch(() => {});
    await wait(POWER_REJECTION_MS);
    powerSwitch.classList.remove("on");
    powerLed.classList.remove("rejected");
    powerLed.classList.add("off");
    updateControlGuides();
    if (message) console.warn("CQA: power rejected", message);
  }

  function updateControlGuides() {
    const needsCart =
      ready &&
      !shellBackVisible &&
      !trayOpen &&
      !batteryTrayOpen &&
      !cartridge &&
      !powered &&
      Boolean(installedProvider);
    const needsPower = ready && !shellBackVisible && !trayOpen && !batteryTrayOpen && !powerTransitioning
      && (Boolean(cartridge) !== powered || (!powered && !installedProvider));
    const needsBatteryTab =
      ready &&
      shellBackVisible &&
      !batteryTrayOpen &&
      !powered &&
      !installedProvider &&
      !batteryDoorOpen;
    const needsBatteryBay =
      ready &&
      shellBackVisible &&
      !batteryTrayOpen &&
      !powered &&
      !installedProvider &&
      batteryDoorOpen;
    cartGuide.classList.toggle("hidden", !needsCart);
    powerGuide.classList.toggle("hidden", !needsPower);
    batteryGuide.classList.toggle("hidden", !needsBatteryTab);
    $("cart-back").classList.toggle("guided", needsCart);
    $("power-switch").classList.toggle("guided", needsPower);
    batteryDoor.classList.toggle("guided", needsBatteryTab);
    batteryBay.classList.toggle("guided", needsBatteryBay);
    const switchingOff = powered && !cartridge;
    const missingBatteries = !powered && !installedProvider;
    const failedProvider = !powered && Boolean(lastPowerFailure);
    powerGuide.classList.toggle("switching-off", switchingOff);
    powerGuide.querySelector(".guide-action").textContent = switchingOff
      ? "TURN POWER OFF"
      : missingBatteries || failedProvider
        ? "CHECK BATTERIES"
        : "TURN POWER ON";
    powerGuide.querySelector(".guide-detail").textContent = switchingOff
      ? "TO LOAD A GAME"
      : missingBatteries
        ? "TURN UNIT OVER"
        : failedProvider
          ? lastPowerFailure
          : "TO START";
    powerGuide.setAttribute(
      "aria-label",
      switchingOff
        ? "Turn the power off to load a game"
        : missingBatteries || failedProvider
          ? "Check the AI provider batteries on the back of the device"
          : "Turn the power on to start",
    );
  }

  function setShellBackVisible(visible) {
    shellBackVisible = Boolean(visible);
    if (!shellBackVisible && batteryDoorOpen) setBatteryDoorOpen(false, { force: true });
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
    if (powerTransitioning || powered === target) return;
    powerTransitioning = true;

    if (target) {
      if (trayOpen) closeTray();
      if (batteryTrayOpen) closeBatteryTray({ restoreFocus: false });
      setBatteryDoorOpen(false, { force: true });
      powerSwitch.classList.add("on", "checking");
      powerLed.classList.remove("off", "rejected");
      powerLed.classList.add("checking");
      powerSwitch.setAttribute("aria-label", "Power switch, checking AI batteries");
      updateControlGuides();
      try {
        await verifyInstalledProvider();
        await invoke("engine_power", { powered: true });
        powered = true;
        powerSwitch.classList.remove("checking");
        powerLed.classList.remove("checking");
        powerSwitch.setAttribute("aria-label", `Power on, ${providerLabel()} batteries ready`);
        batteryCompartment.classList.add("locked");
        showDeviceBoot();
      } catch (error) {
        await rejectPowerOn(error);
      } finally {
        powerTransitioning = false;
        updateControlGuides();
      }
      return;
    }

    powered = false;
    powerSwitch.classList.remove("on", "checking");
    powerLed.classList.remove("checking", "rejected");
    powerLed.classList.add("off");
    batteryCompartment.classList.remove("locked");
    powerSwitch.setAttribute("aria-label", "Power switch, off");
    hideDeviceBoot();
    updateControlGuides();
    try {
      await invoke("engine_power", { powered: false });
    } catch (error) {
      powered = true;
      powerSwitch.classList.add("on");
      powerLed.classList.remove("off");
      batteryCompartment.classList.add("locked");
      showTrayError(error);
    } finally {
      powerTransitioning = false;
      updateControlGuides();
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
    let changed = false;
    for (const update of updates) {
      if (!update) continue;
      const index = cartridges.findIndex(({ path }) => path === update.path);
      if (index < 0 || cartridges[index].branch === update.branch) continue;
      cartridges[index] = { ...cartridges[index], branch: update.branch };
      changed = true;
    }
    if (!changed) return;
    persistCartridges();
    if (trayOpen) buildTray();
  }

  function showTrayMessage(message, tone = "error") {
    const error = $("tray-error");
    if (trayMessageTimer !== null) window.clearTimeout(trayMessageTimer);
    error.textContent = String(message);
    error.classList.toggle("notice", tone === "notice");
    error.classList.remove("hidden");
    trayMessageTimer = window.setTimeout(() => {
      error.classList.add("hidden");
      trayMessageTimer = null;
    }, 4000);
  }

  function showTrayError(message) {
    showTrayMessage(message);
  }

  async function insertCartridge(value) {
    if (!value || cartridge) return;
    const alreadyCached = cartridges.some((entry) => entry.path === value.path);
    if (!alreadyCached && cartridges.length >= MAX_CARTRIDGES) {
      showTrayError("CARTRIDGE RACK FULL · RECYCLE ONE FIRST");
      return;
    }
    const configured = await invoke("engine_set_cartridge", { path: value.path });
    cartridge = configured;
    localStorage.setItem("cqa-cart-id", configured.path);
    if (!cacheCartridge(configured)) {
      await invoke("engine_set_cartridge", { path: null });
      cartridge = null;
      localStorage.removeItem("cqa-cart-id");
      showTrayError("CARTRIDGE RACK FULL · RECYCLE ONE FIRST");
      return;
    }
    renderCartridge();
    closeTray();
  }

  async function insertByPath(path) {
    if (cartridge) return;
    try {
      await insertCartridge({ path });
    } catch (error) {
      forgetCartridge(path);
      buildTray();
      showTrayError(error);
    }
  }

  async function addFromDisk() {
    if (picking || cartridge) return;
    if (cartridges.length >= MAX_CARTRIDGES) {
      showTrayError("CARTRIDGE RACK FULL · RECYCLE ONE FIRST");
      return;
    }
    picking = true;
    try {
      await insertCartridge(await invoke("pick_cartridge"));
    } catch (error) {
      showTrayError(error);
    } finally {
      picking = false;
    }
  }

  function ejectCartridge() {
    if (!cartridge) return;
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
        showTrayError(error);
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
      showTrayError("EJECT THIS CARTRIDGE BEFORE RECYCLING IT");
      return;
    }
    card.classList.add("recycling");
    card.disabled = true;
    window.setTimeout(() => {
      forgetCartridge(value.path);
      if (trayOpen) buildTray();
      showTrayMessage(`RECYCLED ${value.title} · REPOSITORY UNTOUCHED`, "notice");
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
      else if (deniedLoad) showTrayError("EJECT THE CURRENT CARTRIDGE BEFORE LOADING ANOTHER");
      else if (deniedRecycle) showTrayError("EJECT THIS CARTRIDGE BEFORE RECYCLING IT");
      else if (!moved && !cartridge) insertByPath(value.path);
    });

    card.addEventListener("pointercancel", clearDrag);
    card.addEventListener("keydown", (event) => {
      if ((event.key === "Enter" || event.key === " ") && !cartridge) {
        event.preventDefault();
        insertByPath(value.path);
      } else if ((event.key === "Delete" || event.key === "Backspace") && !current) {
        event.preventDefault();
        recycleCartridge(value, card);
      }
    });
  }

  function buildTray() {
    const list = $("tray-carts");
    list.innerHTML = "";
    for (const value of cartridges) {
      const card = document.createElement("button");
      const current = cartridge?.path === value.path;
      card.type = "button";
      card.className = `cart-card${current ? " current" : ""}`;
      const accessibilityAction = current
        ? "Currently in the device. Use the Eject Cartridge control before recycling."
        : cartridge
          ? "Drag down or press Delete to recycle. Eject the current cartridge before loading."
          : "Drag up or press Enter to load. Drag down or press Delete to recycle.";
      card.setAttribute(
        "aria-label",
        `${value.title}, branch ${value.branch}. ${accessibilityAction}`,
      );
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
    document.querySelector(".tray-head").textContent = `CARTRIDGE RACK · ${cartridges.length}/${MAX_CARTRIDGES}`;
    $("tray-error").classList.add("hidden");
    document.querySelector(".tray-hint").textContent = cartridge
      ? "EJECT CURRENT · DRAG OTHER CARTS DOWN TO RECYCLE"
      : "DRAG ↑ TO LOAD · DRAG ↓ TO RECYCLE · ESC TO CLOSE";
  }

  function openTray() {
    if (powered) return;
    if (batteryTrayOpen) closeBatteryTray({ restoreFocus: false });
    buildTray();
    $("cart-tray").classList.remove("hidden");
    trayOpen = true;
    refreshCartridgeBranches().catch(() => {});
    updateControlGuides();
  }

  function closeTray() {
    $("cart-tray").classList.add("hidden");
    trayOpen = false;
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
    document.querySelector(".battery-tray-hint").textContent = hasProvider
      ? "EJECT CURRENT PACK BEFORE LOADING ANOTHER"
      : "SELECT A PACK · ESC TO CLOSE";
  }

  function openBatteryTray() {
    if (powered || batteryChanging || !batteryDoorOpen) return;
    if (trayOpen) closeTray();
    renderBatteryTray();
    batteryTray.classList.remove("hidden");
    batteryTray.setAttribute("aria-hidden", "false");
    batteryTrayOpen = true;
    const firstAction = installedProvider
      ? batteryEject
      : batteryOptions.querySelector("[data-provider]");
    firstAction?.focus();
    updateControlGuides();
  }

  function closeBatteryTray({ restoreFocus = true } = {}) {
    batteryTray.classList.add("hidden");
    batteryTray.setAttribute("aria-hidden", "true");
    batteryTrayOpen = false;
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
        const choices = [...batteryOptions.querySelectorAll("button:not(:disabled)")];
        const firstChoice = choices[0];
        const lastChoice = choices.at(-1);
        if (event.shiftKey && document.activeElement === firstChoice) {
          event.preventDefault();
          lastChoice.focus();
        } else if (!event.shiftKey && document.activeElement === lastChoice) {
          event.preventDefault();
          firstChoice.focus();
        }
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
    if (trayOpen && (event.key === "Escape" || event.code === "KeyS")) {
      closeTray();
      return;
    }
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

  viewToggle.addEventListener("click", (event) => {
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
      });
    });
  });
  batteryEject.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    if (!installedProvider) return;
    setInstalledProvider(null).then((changed) => {
      if (changed) batteryOptions.querySelector("[data-provider]")?.focus();
    });
  });

  batteryGuide.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    setBatteryDoorOpen(true);
  });

  $("power-switch").addEventListener("pointerdown", (event) => {
    event.preventDefault();
    event.stopPropagation();
    setPower(!powered);
  });
  $("power-switch").addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    setPower(!powered);
  });
  $("cart-back").addEventListener("pointerdown", (event) => {
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

  async function initialize() {
    fit();
    setShellBackVisible(false);
    rearSerial.textContent = await invoke("app_revision");
    installedProvider = normalizeProvider(localStorage.getItem(PROVIDER_STORAGE_KEY));
    if (!installedProvider) localStorage.removeItem(PROVIDER_STORAGE_KEY);
    verifiedProvider = null;
    renderProviderBatteries();
    setBatteryDoorOpen(false, { force: true });
    try {
      await invoke("engine_set_ai_provider", { provider: installedProvider });
    } catch (error) {
      installedProvider = null;
      localStorage.removeItem(PROVIDER_STORAGE_KEY);
      renderProviderBatteries();
      showTrayError(error);
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
      } catch (_) {
        cartridge = null;
        localStorage.removeItem("cqa-cart-id");
        forgetCartridge(savedPath);
      }
    }
    renderCartridge();
    await invoke("engine_power", { powered: false });
    ready = true;
    updateControlGuides();
    window.requestAnimationFrame(drawFrame);
  }

  initialize().catch((error) => showTrayError(error));

  function createBrowserDemo() {
    const frame = new Uint8Array(FRAME_BYTES);
    for (let offset = 0; offset < frame.length; offset += 4) {
      frame.set([26, 28, 44, 255], offset);
    }
    return async (command, args) => {
      if (command === "engine_frame") return frame.buffer;
      if (command === "app_revision") return "0000000";
      if (["engine_power", "engine_finish_boot", "engine_input", "engine_set_ai_provider"].includes(command)) return null;
      if (command === "verify_ai_provider") return { provider: args?.provider, ready: true };
      if (command === "engine_set_cartridge" && args?.path == null) return null;
      if (command === "engine_set_cartridge") throw new Error("RUN IN TAURI TO LOAD CARTRIDGES");
      if (command === "pick_cartridge") return null;
      if (command === "cartridge_branch") return "BRANCH UNKNOWN";
      throw new Error(`UNKNOWN COMMAND ${command}`);
    };
  }
})();
