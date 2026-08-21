/* ============================================================
   FILESYSTEM TOWN — deterministic map generation for repo carts
   Kept framework-free so the same generator can be tested in Node.
   ============================================================ */
"use strict";
(() => {
  const WIDTH = 30;
  const HEIGHT = 20;
  const ROAD_Y = 10;
  const SLOT_X = [1, 7, 13, 19, 25];
  const MAX_LANDMARKS = SLOT_X.length * 2;

  const fnv1a = (value) => {
    let hash = 0x811c9dc5;
    const text = String(value);
    for (let i = 0; i < text.length; i++) {
      hash ^= text.charCodeAt(i);
      hash = Math.imul(hash, 0x01000193);
    }
    return hash >>> 0;
  };

  const normalizedName = (path) => {
    const name = String(path || "REPO").split("/").filter(Boolean).pop() || "REPO";
    const words = name.replace(/^\./, "DOT ").replace(/[-_.]+/g, " ").trim().toUpperCase();
    return words || "REPO";
  };

  const displayName = (path) => normalizedName(path).slice(0, 12);

  const compactName = (name, maxLength = 12) => {
    const words = String(name || "REPO").split(/\s+/).filter(Boolean);
    let result = "";
    for (const word of words) {
      const candidate = result ? `${result} ${word}` : word;
      if (candidate.length > maxLength) break;
      result = candidate;
    }
    return result || String(name || "REPO").slice(0, maxLength);
  };

  const extensionOf = (path) => {
    const base = String(path).split("/").pop() || "";
    const dot = base.lastIndexOf(".");
    return dot > 0 ? base.slice(dot).toLowerCase() : "";
  };

  const kindForFiles = (paths, group) => {
    const lower = paths.map((path) => String(path).toLowerCase());
    const groupName = String(group).toLowerCase();
    if (/(^|[-_])(test|tests|spec|specs)([-_]|$)/.test(groupName) || lower.some((path) => /(^|\/)(test|tests|spec|specs)(\/|$)/.test(path))) return "tests";
    if (/(docs?|guides?)/.test(groupName) || lower.some((path) => /\.(md|mdx|rst|txt)$/.test(path))) return "docs";
    if (/(assets?|public|static|images?|icons?)/.test(groupName) || lower.some((path) => /\.(png|jpe?g|gif|svg|webp|ico|woff2?|ttf)$/.test(path))) return "assets";
    if (lower.some((path) => /(^|\/)(package\.json|cargo\.toml|makefile|dockerfile|[^/]+\.(ya?ml|toml|json))$/.test(path))) return "config";
    if (lower.some((path) => /\.(rs|js|mjs|cjs|ts|tsx|jsx|py|go|rb|java|kt|c|cc|cpp|h|hpp|sh|css|html)$/.test(path))) return "code";
    return group === "$root" ? "root" : "archive";
  };

  const groupFiles = (files) => {
    const groups = new Map();
    for (const entry of Array.isArray(files) ? files : []) {
      const path = typeof entry === "string" ? entry : entry && entry.path;
      if (!path || typeof path !== "string") continue;
      const slash = path.indexOf("/");
      const key = slash < 0 ? "$root" : path.slice(0, slash);
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(path);
    }
    return [...groups.entries()]
      .map(([path, paths]) => ({ path, paths: [...paths].sort() }))
      .sort((a, b) => {
        if (a.path === "$root") return -1;
        if (b.path === "$root") return 1;
        return b.paths.length - a.paths.length || (a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
      })
      .slice(0, MAX_LANDMARKS);
  };

  function createTown(repoName, files) {
    const groups = groupFiles(files);
    if (!groups.length) groups.push({ path: "$root", paths: [] });
    const townLabel = compactName(normalizedName(repoName));
    const landmarks = groups.map((group, index) => {
      const row = index < SLOT_X.length ? 0 : 1;
      const column = index % SLOT_X.length;
      const x = SLOT_X[column];
      const y = row === 0 ? 2 : 14;
      const outsideY = row === 0 ? y + 4 : y - 1;
      const name = group.path === "$root" ? `${townLabel} HALL` : displayName(group.path);
      return {
        id: group.path,
        name: name.slice(0, 16),
        path: group.path === "$root" ? "/" : `${group.path}/`,
        kind: kindForFiles(group.paths, group.path),
        fileCount: group.paths.length,
        x,
        y,
        width: 4,
        height: 4,
        door: { x: x + 2, y: outsideY },
        seed: fnv1a(`${repoName}:${group.path}:${group.paths.map(extensionOf).join(",")}`),
      };
    });
    return {
      name: `${townLabel} TOWN`,
      width: WIDTH,
      height: HEIGHT,
      roadY: ROAD_Y,
      seed: fnv1a(repoName),
      start: { x: 15, y: ROAD_Y },
      landmarks,
    };
  }

  const stepAxis = (route, point, axis, target) => {
    while (point[axis] !== target) {
      point = { ...point, [axis]: point[axis] + (point[axis] < target ? 1 : -1) };
      route.push(point);
    }
    return point;
  };

  function routeBetween(town, from, to) {
    const width = town && town.width || WIDTH;
    const height = town && town.height || HEIGHT;
    const roadY = town && Number.isInteger(town.roadY) ? town.roadY : ROAD_Y;
    const safe = (point) => ({
      x: Math.max(0, Math.min(width - 1, Number(point && point.x) || 0)),
      y: Math.max(0, Math.min(height - 1, Number(point && point.y) || 0)),
    });
    let point = safe(from);
    const target = safe(to);
    const route = [{ ...point }];
    point = stepAxis(route, point, "y", roadY);
    point = stepAxis(route, point, "x", target.x);
    stepAxis(route, point, "y", target.y);
    return route;
  }

  globalThis.CodeQuestTown = Object.freeze({
    createTown,
    routeBetween,
    kindForFiles,
  });
})();
