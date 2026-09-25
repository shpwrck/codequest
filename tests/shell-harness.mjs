/* Runs the real device adapter (src/main.js) against a minimal fake DOM, a
 * manual clock, and a scripted Tauri bridge, so contracts can drive the shell
 * through real event handlers and hold backend calls open to test races.
 * This is a helper module, not a contract: npm test does not run it alone. */

let scenarios = 0;

class FakeClassList {
  constructor() { this.names = new Set(); }
  add(...names) { for (const name of names) this.names.add(name); }
  remove(...names) { for (const name of names) this.names.delete(name); }
  contains(name) { return this.names.has(name); }
  toggle(name, force) {
    const on = force === undefined ? !this.names.has(name) : Boolean(force);
    if (on) this.names.add(name);
    else this.names.delete(name);
    return on;
  }
}

function fakeEvent(target, init = {}) {
  return {
    target,
    button: 0,
    pointerId: 1,
    repeat: false,
    shiftKey: false,
    deltaMode: 0,
    deltaY: 0,
    clientY: 0,
    defaultPrevented: false,
    preventDefault() { this.defaultPrevented = true; },
    stopPropagation() {},
    ...init,
  };
}

function createDom() {
  const document = { activeElement: null };

  class FakeElement {
    constructor(tagName = "div", id = "") {
      this.tagName = tagName.toUpperCase();
      this.id = id;
      this.classList = new FakeClassList();
      this.attributes = new Map();
      this.dataset = {};
      this.style = { setProperty() {}, removeProperty() {} };
      this.children = [];
      this.parentElement = null;
      this.listeners = new Map();
      this.selected = new Map();
      this.textContent = "";
      this.disabled = false;
      this.inert = false;
      this.offsetWidth = 0;
      this.markup = "";
    }
    get className() { return [...this.classList.names].join(" "); }
    set className(value) {
      this.classList = new FakeClassList();
      this.classList.add(...String(value).split(/\s+/).filter(Boolean));
    }
    get innerHTML() { return this.markup; }
    set innerHTML(value) {
      this.markup = String(value);
      if (!this.markup) {
        for (const child of this.children) child.parentElement = null;
        this.children = [];
      }
    }
    setAttribute(name, value) { this.attributes.set(name, String(value)); }
    getAttribute(name) { return this.attributes.has(name) ? this.attributes.get(name) : null; }
    appendChild(child) {
      child.parentElement = this;
      this.children.push(child);
      return child;
    }
    contains(other) {
      for (let node = other; node; node = node.parentElement) if (node === this) return true;
      return false;
    }
    matches() { return this.tagName === "BUTTON"; }
    focus() { document.activeElement = this; }
    setPointerCapture() {}
    getContext() {
      return {
        imageSmoothingEnabled: true,
        createImageData: (width, height) => ({ data: new Uint8ClampedArray(width * height * 4) }),
        putImageData() {},
      };
    }
    querySelector(selector) {
      if (selector.startsWith("button")) {
        return this.children.find((child) => child.tagName === "BUTTON" && !child.disabled) ?? null;
      }
      if (!this.selected.has(selector)) this.selected.set(selector, new FakeElement("div"));
      return this.selected.get(selector);
    }
    querySelectorAll() { return []; }
    addEventListener(type, handler) {
      if (!this.listeners.has(type)) this.listeners.set(type, []);
      this.listeners.get(type).push(handler);
    }
    dispatch(type, init) {
      const event = fakeEvent(this, init);
      for (const handler of this.listeners.get(type) ?? []) handler(event);
      return event;
    }
  }

  const elements = new Map();
  const queried = new Map();
  document.body = new FakeElement("body");
  document.activeElement = document.body;
  document.getElementById = (id) => {
    if (!elements.has(id)) elements.set(id, new FakeElement("div", id));
    return elements.get(id);
  };
  document.querySelector = (selector) => {
    if (!queried.has(selector)) queried.set(selector, new FakeElement("div"));
    return queried.get(selector);
  };
  document.querySelectorAll = () => [];
  document.createElement = (tagName) => new FakeElement(tagName);
  return { document, FakeElement };
}

function memoryStorage(initial) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: (key) => (values.has(key) ? values.get(key) : null),
    setItem: (key, value) => values.set(key, String(value)),
    removeItem: (key) => values.delete(key),
    values,
  };
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

/**
 * Boots a fresh copy of the adapter. `storage` seeds localStorage and
 * `hold` lists commands whose first call stays pending until the test
 * resolves or rejects it through `pending(command)`.
 */
export async function bootShell({ storage = {}, hold = [], respond = {} } = {}) {
  const { document, FakeElement } = createDom();
  const localStorage = memoryStorage(storage);
  const calls = [];
  const held = new Map();
  const holding = new Set(hold);
  const timers = [];
  let frames = [];
  let now = 0;
  let timerId = 0;

  const defaults = {
    app_revision: () => "abc1234",
    engine_frame: () => new ArrayBuffer(0),
    engine_audio: () => ({ tick: 0, notes: [] }),
    verify_ai_provider: (args) => ({ provider: args.provider, ready: true }),
    cartridge_branch: () => "main",
    engine_set_cartridge: (args) => (args.path == null ? null : {
      path: args.path,
      title: args.path.split("/").at(-1).toUpperCase(),
      branch: "main",
      revision: "abc1234",
      color: "#6a6fd1",
    }),
  };

  function invoke(command, args = {}) {
    calls.push({ command, args });
    if (holding.has(command)) {
      holding.delete(command);
      return new Promise((resolve, reject) => held.set(command, { resolve, reject }));
    }
    try {
      const answer = respond[command] ?? defaults[command] ?? (() => null);
      return Promise.resolve(answer(args));
    } catch (error) {
      return Promise.reject(error);
    }
  }

  const windowListeners = new Map();
  const window = {
    innerWidth: 1280,
    innerHeight: 800,
    __TAURI__: { core: { invoke } },
    matchMedia: () => ({ matches: false, addEventListener() {} }),
    addEventListener(type, handler) {
      if (!windowListeners.has(type)) windowListeners.set(type, []);
      windowListeners.get(type).push(handler);
    },
    setTimeout(handler, delay = 0) {
      timerId += 1;
      timers.push({ id: timerId, at: now + delay, handler });
      return timerId;
    },
    clearTimeout(id) {
      const index = timers.findIndex((timer) => timer.id === id);
      if (index >= 0) timers.splice(index, 1);
    },
    requestAnimationFrame(handler) { frames.push(handler); },
  };

  const previous = Object.fromEntries(
    ["window", "document", "localStorage", "HTMLElement", "Element"].map((name) => [
      name,
      Object.getOwnPropertyDescriptor(globalThis, name),
    ]),
  );
  const define = (name, value) => Object.defineProperty(globalThis, name, {
    value,
    configurable: true,
    writable: true,
  });
  define("window", window);
  define("document", document);
  define("localStorage", localStorage);
  define("HTMLElement", FakeElement);
  define("Element", FakeElement);

  scenarios += 1;
  await import(new URL(`../src/main.js?scenario=${scenarios}`, import.meta.url));

  async function flush() {
    for (let round = 0; round < 8; round += 1) await settle();
  }

  const shell = {
    calls,
    storage: localStorage,
    el: (id) => document.getElementById(id),
    calledWith(command, predicate = () => true) {
      return calls.filter((call) => call.command === command && predicate(call.args ?? {}));
    },
    /** Settles a held call; `value` may be an Error to reject it. */
    async answer(command, value) {
      const call = held.get(command);
      if (!call) throw new Error(`${command} is not being held`);
      held.delete(command);
      if (value instanceof Error) call.reject(value.message);
      else call.resolve(value);
      await flush();
    },
    isHeld: (command) => held.has(command),
    /** Holds the next call to `command` open until `answer(command, ...)`. */
    hold(command) { holding.add(command); },
    async key(code, key = code) {
      for (const handler of windowListeners.get("keydown") ?? []) {
        handler(fakeEvent(document.body, { code, key }));
      }
      for (const handler of windowListeners.get("keyup") ?? []) {
        handler(fakeEvent(document.body, { code, key }));
      }
      await flush();
    },
    async dispatch(element, type, init) {
      const event = element.dispatch(type, init);
      await flush();
      return event;
    },
    /** Runs queued animation frames (twice covers waitForPaint). */
    async paint(rounds = 2) {
      for (let round = 0; round < rounds; round += 1) {
        const due = frames;
        frames = [];
        for (const handler of due) handler(now);
        await flush();
      }
    },
    /** Advances the manual clock, firing due timers in order. */
    async advance(milliseconds) {
      const until = now + milliseconds;
      for (;;) {
        timers.sort((left, right) => left.at - right.at);
        if (!timers.length || timers[0].at > until) break;
        const timer = timers.shift();
        now = timer.at;
        timer.handler();
        await flush();
      }
      now = until;
      await flush();
    },
    get powered() { return document.getElementById("power-switch").classList.contains("on"); },
    get loaded() { return document.getElementById("cart-back").classList.contains("loaded"); },
    get trayOpen() { return document.getElementById("cart-tray").getAttribute("aria-hidden") === "false"; },
    /** Every message surface's current text, joined. */
    get messages() {
      return ["device-message", "tray-error", "battery-tray-error"]
        .map((id) => document.getElementById(id).textContent)
        .filter(Boolean)
        .join(" | ");
    },
    rackCard: (index = 0) => document.getElementById("tray-carts").children[index],
    rackAction: (name) => document.getElementById("tray-carts").children
      .find((card) => card.classList.contains(name)),
    close() {
      for (const [name, descriptor] of Object.entries(previous)) {
        if (descriptor) Object.defineProperty(globalThis, name, descriptor);
        else delete globalThis[name];
      }
    },
  };
  await flush();
  return shell;
}

/** A saved rack with one or more repositories, as localStorage stores it. */
export function rackStorage(paths, { current = null, provider = "codex" } = {}) {
  const storage = {
    "cqa-repo-carts": JSON.stringify(paths.map((path) => ({
      path,
      title: path.split("/").at(-1).toUpperCase(),
      branch: "main",
      revision: "abc1234",
      color: "#6a6fd1",
    }))),
  };
  if (current) storage["cqa-cart-id"] = current;
  if (provider) storage["cqa-ai-provider"] = provider;
  return storage;
}
