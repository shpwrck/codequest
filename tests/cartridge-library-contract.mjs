import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  MAX_CARTRIDGES,
  cartridgeDragIntent,
  normalizeCartridges,
  upsertCartridge,
} from "../src/cartridge-library.js";

const cartridge = (path, title = path.toUpperCase()) => ({
  path,
  title,
  color: "#6a6fd1",
});

assert.equal(MAX_CARTRIDGES, 3, "The physical rack must have exactly three slots");

assert.deepEqual(
  normalizeCartridges([
    cartridge("/one"),
    cartridge("/two"),
    cartridge("/one", "UPDATED"),
    cartridge("/three"),
    cartridge("/four"),
    null,
  ]).map(({ path }) => path),
  ["/one", "/two", "/three"],
  "Stored cartridges should be deduplicated and capped",
);

assert.deepEqual(
  normalizeCartridges(
    [cartridge("/one"), cartridge("/two"), cartridge("/three"), cartridge("/four")],
    "/four",
  ).map(({ path }) => path),
  ["/four", "/one", "/two"],
  "The currently inserted cartridge should survive migration from an oversized library",
);
assert.deepEqual(
  normalizeCartridges(
    [cartridge("/one"), cartridge("/two"), cartridge("/three"), cartridge("/four")],
    "/missing",
  ).map(({ path }) => path),
  ["/one", "/two", "/three"],
  "An invalid saved path must not displace a usable cartridge during migration",
);

const fullRack = [cartridge("/one"), cartridge("/two"), cartridge("/three")];
assert.equal(
  upsertCartridge(fullRack, cartridge("/four")).accepted,
  false,
  "A fourth cartridge must not displace an existing one",
);
const refreshed = upsertCartridge(fullRack, cartridge("/two", "REFRESHED"));
assert.equal(refreshed.accepted, true, "Refreshing an existing cartridge remains allowed");
assert.equal(refreshed.items[1].title, "REFRESHED");
assert.equal(
  upsertCartridge([], { path: "/unsafe", title: "UNSAFE", color: "red; opacity: 0" })
    .items[0].color,
  "#6a6fd1",
  "Persisted label colors must not inject arbitrary inline styles",
);

assert.equal(cartridgeDragIntent(-50), "load", "Dragging up should load");
assert.equal(cartridgeDragIntent(50), "recycle", "Dragging down should recycle");
assert.equal(cartridgeDragIntent(-20), null, "Small movement should remain a click");
assert.equal(
  cartridgeDragIntent(-50, { canLoad: false }),
  null,
  "A loaded device should reject another upward insertion",
);
assert.equal(
  cartridgeDragIntent(50, { canRecycle: false }),
  null,
  "The inserted cartridge cannot be recycled in place",
);

const mainSource = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const styles = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
assert.doesNotMatch(
  mainSource,
  /(?:EMPTY RACK|DEVICE) SLOT/,
  "Action tiles must not leak cartridge-slot strip copy",
);
assert.match(styles, /\.tray-head \{ font-size: 12px;/);
assert.match(styles, /\.tray-hint \{[^}]*font-size: 12px;/);
assert.match(styles, /\.tray-safety \{[^}]*font-size: 12px;/);
assert.match(styles, /\.cc-sub \{[^}]*font-size: 9px;/);
assert.match(styles, /\.cc-gesture \{[^}]*font-size: 9px;/);
assert.match(styles, /\.cc-sub \{[^}]*white-space: nowrap;/);
assert.match(styles, /\.cc-gesture \{[^}]*white-space: nowrap;/);
assert.match(mainSource, /current \? "EJECT FIRST" : "↑ LOAD · ↓ RECYCLE"/);

console.log("Cartridge library contract OK: three slots with up/load and down/recycle gestures");
