import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const engineDir = fileURLToPath(new URL("../src-tauri/src/engine/", import.meta.url));

function rustFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return rustFiles(path);
    return entry.name.endsWith(".rs") ? [path] : [];
  });
}

/**
 * The engine's Rust source: engine/mod.rs, then every other engine file in
 * path order (test_support.rs excluded). With `production`, each file is cut
 * at its `#[cfg(test)] mod tests` module.
 */
export function engineSource({ production = false } = {}) {
  const root = join(engineDir, "mod.rs");
  const files = rustFiles(engineDir)
    .filter((path) => path !== root && !path.endsWith("test_support.rs"))
    .sort();
  return [root, ...files]
    .map((path) => {
      const text = readFileSync(path, "utf8");
      if (!production) return text;
      const cut = text.search(/^#\[cfg\(test\)\]\r?\nmod tests\b/m);
      return cut < 0 ? text : text.slice(0, cut);
    })
    .join("\n");
}
