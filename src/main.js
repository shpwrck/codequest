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
  const context = canvas.getContext("2d", { alpha: false });
  context.imageSmoothingEnabled = false;
  const image = context.createImageData(WIDTH, HEIGHT);

  let powered = false;
  let ready = false;
  let cartridge = null;
  let cartridges = [];
  let trayOpen = false;
  let picking = false;
  let framePending = false;
  let bootTimer = null;
  let bootStartedAt = 0;
  let bootFinishing = false;
  let bootGeneration = 0;
  let trayMessageTimer = null;
  let shellBackVisible = false;
  let shellTurning = false;
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

  function updateControlGuides() {
    const needsCart = ready && !shellBackVisible && !trayOpen && !cartridge && !powered;
    const needsPower = ready && !shellBackVisible && !trayOpen && (Boolean(cartridge) !== powered);
    cartGuide.classList.toggle("hidden", !needsCart);
    powerGuide.classList.toggle("hidden", !needsPower);
    $("cart-back").classList.toggle("guided", needsCart);
    $("power-switch").classList.toggle("guided", needsPower);
    const switchingOff = powered && !cartridge;
    powerGuide.classList.toggle("switching-off", switchingOff);
    powerGuide.querySelector(".guide-action").textContent = switchingOff ? "TURN POWER OFF" : "TURN POWER ON";
    powerGuide.querySelector(".guide-detail").textContent = switchingOff ? "TO LOAD A GAME" : "TO START";
    powerGuide.setAttribute("aria-label", switchingOff ? "Turn the power off to load a game" : "Turn the power on to start");
  }

  function setShellBackVisible(visible) {
    shellBackVisible = Boolean(visible);
    scaleEl.classList.toggle("showing-back", shellBackVisible);
    frontFace.setAttribute("aria-hidden", String(shellBackVisible));
    backFace.setAttribute("aria-hidden", String(!shellBackVisible));
    frontFace.inert = shellBackVisible;
    backFace.inert = !shellBackVisible;
    updateControlGuides();
  }

  function turnShell({ moveFocus = false } = {}) {
    if (shellTurning) return;
    const nextBackVisible = !shellBackVisible;
    const swapFaces = () => {
      setShellBackVisible(nextBackVisible);
      if (moveFocus) {
        const visibleFace = nextBackVisible ? backFace : frontFace;
        visibleFace.querySelector("[data-device-turn]")?.focus({ preventScroll: true });
      }
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
    if (powered === on) return;
    powered = on;
    $("power-switch").classList.toggle("on", on);
    document.querySelector(".power-led").classList.toggle("off", !on);
    if (on && trayOpen) closeTray();
    if (on) showDeviceBoot();
    else hideDeviceBoot();
    updateControlGuides();
    try {
      await invoke("engine_power", { powered: on });
    } catch (error) {
      powered = !on;
      $("power-switch").classList.toggle("on", powered);
      document.querySelector(".power-led").classList.toggle("off", !powered);
      if (on) hideDeviceBoot();
      updateControlGuides();
      showTrayError(error);
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
    const metadata = cartridges.map(({ path, title, branch, color }) => ({
      path,
      title,
      branch,
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

  document.querySelectorAll("[data-device-turn]").forEach((element) => {
    element.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const shouldMoveFocus = document.activeElement === element;
      turnShell({ moveFocus: shouldMoveFocus });
    });
  });

  $("power-switch").addEventListener("pointerdown", (event) => {
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
  window.addEventListener("resize", fit);

  async function initialize() {
    fit();
    setShellBackVisible(false);
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
      if (["engine_power", "engine_finish_boot", "engine_input"].includes(command)) return null;
      if (command === "engine_set_cartridge" && args?.path == null) return null;
      if (command === "engine_set_cartridge") throw new Error("RUN IN TAURI TO LOAD CARTRIDGES");
      if (command === "pick_cartridge") return null;
      if (command === "cartridge_branch") return "BRANCH UNKNOWN";
      throw new Error(`UNKNOWN COMMAND ${command}`);
    };
  }
})();
