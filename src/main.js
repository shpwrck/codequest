/* CODE QUEST ADVANCE device adapter.
 * The browser owns only the shell. Bevy owns game state, timing, process
 * execution, and the fixed 240x160 RGBA framebuffer exposed by Rust. */
"use strict";

(() => {
  const WIDTH = 240;
  const HEIGHT = 160;
  const FRAME_BYTES = WIDTH * HEIGHT * 4;
  const BOOT_DURATION_MS = 2600;
  const BOOT_SKIP_DELAY_MS = 650;
  const $ = (id) => document.getElementById(id);
  const tauri = window.__TAURI__;
  const invoke = tauri?.core?.invoke
    ? (command, args) => tauri.core.invoke(command, args)
    : createBrowserDemo();

  const scaleEl = $("shell-scale");
  const canvas = $("engine-canvas");
  const bootOverlay = $("device-boot");
  const context = canvas.getContext("2d", { alpha: false });
  context.imageSmoothingEnabled = false;
  const image = context.createImageData(WIDTH, HEIGHT);

  let powered = false;
  let cartridge = null;
  let cartridges = [];
  let trayOpen = false;
  let picking = false;
  let framePending = false;
  let bootTimer = null;
  let bootStartedAt = 0;
  let bootFinishing = false;
  let bootGeneration = 0;
  const held = Object.create(null);
  const swallowedByBoot = Object.create(null);

  function fit() {
    const available = Math.min(window.innerWidth / 584, window.innerHeight / 352);
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

  async function setPower(on) {
    if (powered === on) return;
    powered = on;
    $("power-switch").classList.toggle("on", on);
    document.querySelector(".power-led").classList.toggle("off", !on);
    if (on && trayOpen) closeTray();
    if (on) showDeviceBoot();
    else hideDeviceBoot();
    try {
      await invoke("engine_power", { powered: on });
    } catch (error) {
      powered = !on;
      $("power-switch").classList.toggle("on", powered);
      document.querySelector(".power-led").classList.toggle("off", !powered);
      if (on) hideDeviceBoot();
      showTrayError(error);
    }
  }

  function renderCartridge() {
    const slot = $("cart-back");
    if (cartridge) {
      slot.className = "loaded";
      slot.style.setProperty("--cart-color", cartridge.color || "#6a6fd1");
      slot.title = `CARTRIDGE: ${cartridge.title}`;
    } else {
      slot.className = "empty";
      slot.style.removeProperty("--cart-color");
      slot.title = "CARTRIDGE SLOT (EMPTY)";
    }
  }

  function persistCartridges() {
    const metadata = cartridges.map(({ path, title, color }) => ({ path, title, color }));
    localStorage.setItem("cqa-repo-carts", JSON.stringify(metadata));
  }

  function cacheCartridge(value) {
    const metadata = { path: value.path, title: value.title, color: value.color };
    const index = cartridges.findIndex((entry) => entry.path === value.path);
    if (index >= 0) cartridges[index] = metadata;
    else cartridges.push(metadata);
    persistCartridges();
  }

  function forgetCartridge(path) {
    cartridges = cartridges.filter((entry) => entry.path !== path);
    persistCartridges();
  }

  function showTrayError(message) {
    const error = $("tray-error");
    error.textContent = String(message);
    error.classList.remove("hidden");
    window.setTimeout(() => error.classList.add("hidden"), 4000);
  }

  async function insertCartridge(value) {
    if (!value || cartridge) return;
    const configured = await invoke("engine_set_cartridge", { path: value.path });
    cartridge = configured;
    localStorage.setItem("cqa-cart-id", configured.path);
    cacheCartridge(configured);
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

  function buildTray() {
    const list = $("tray-carts");
    list.innerHTML = "";
    for (const value of cartridges) {
      const card = document.createElement("div");
      const current = cartridge?.path === value.path;
      card.className = `cart-card${current ? " current" : ""}${cartridge ? " locked" : ""}`;
      const shortPath = value.path.length > 26 ? `…${value.path.slice(-25)}` : value.path;
      card.innerHTML = `<div class="cc-strip">CODEQUEST ADVANCE</div><div class="cc-label" style="--cc:${escapeHtml(value.color || "#6a6fd1")}"><span class="cc-title">${escapeHtml(value.title)}</span><span class="cc-sub">${escapeHtml(shortPath)}</span></div>`;
      if (!cartridge) card.addEventListener("pointerdown", (event) => {
        event.stopPropagation();
        insertByPath(value.path);
      });
      list.appendChild(card);
    }

    const add = document.createElement("div");
    add.className = `cart-card add${cartridge ? " locked" : ""}`;
    add.innerHTML = `<div class="cc-strip">&nbsp;</div><div class="cc-label"><span class="cc-title">+ ADD FROM DISK</span><span class="cc-sub">PICK A GIT REPO</span></div>`;
    if (!cartridge) add.addEventListener("pointerdown", (event) => {
      event.stopPropagation();
      addFromDisk();
    });
    list.appendChild(add);

    const eject = document.createElement("div");
    eject.className = `cart-card eject${cartridge ? "" : " locked"}`;
    eject.innerHTML = `<div class="cc-strip">&nbsp;</div><div class="cc-label"><span class="cc-title">EMPTY SLOT</span><span class="cc-sub">${cartridge ? "EJECT CARTRIDGE" : "NO CART LOADED"}</span></div>`;
    if (cartridge) eject.addEventListener("pointerdown", (event) => {
      event.stopPropagation();
      ejectCartridge();
    });
    list.appendChild(eject);
    $("tray-error").classList.add("hidden");
    document.querySelector(".tray-hint").textContent = cartridge
      ? "EJECT THE CARTRIDGE BEFORE SWAPPING"
      : "CARTRIDGES ARE LOCAL GIT REPOS · ESC TO CLOSE";
  }

  function openTray() {
    if (powered) return;
    buildTray();
    $("cart-tray").classList.remove("hidden");
    trayOpen = true;
  }

  function closeTray() {
    $("cart-tray").classList.add("hidden");
    trayOpen = false;
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

  $("power-switch").addEventListener("pointerdown", (event) => {
    event.stopPropagation();
    setPower(!powered);
  });
  $("cart-back").addEventListener("pointerdown", (event) => {
    event.stopPropagation();
    trayOpen ? closeTray() : openTray();
  });
  $("cart-tray").addEventListener("pointerdown", (event) => {
    if (event.target === $("cart-tray")) closeTray();
  });
  window.addEventListener("resize", fit);

  async function initialize() {
    fit();
    try {
      cartridges = (JSON.parse(localStorage.getItem("cqa-repo-carts")) || [])
        .filter((entry) => entry && typeof entry.path === "string");
    } catch (_) {
      cartridges = [];
    }
    const savedPath = localStorage.getItem("cqa-cart-id");
    if (savedPath) {
      try {
        cartridge = await invoke("engine_set_cartridge", { path: savedPath });
        cacheCartridge(cartridge);
      } catch (_) {
        cartridge = null;
        localStorage.removeItem("cqa-cart-id");
        forgetCartridge(savedPath);
      }
    }
    renderCartridge();
    await invoke("engine_power", { powered: false });
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
      throw new Error(`UNKNOWN COMMAND ${command}`);
    };
  }
})();
