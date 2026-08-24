export const MAX_CARTRIDGES = 3;
export const CARTRIDGE_DRAG_THRESHOLD = 44;

function cartridgeMetadata(value) {
  if (!value || typeof value.path !== "string" || !value.path.trim()) return null;
  const path = value.path;
  const fallbackTitle = path.split(/[\\/]/).filter(Boolean).at(-1) || "UNTITLED";
  const branch = typeof value.branch === "string"
    ? value.branch.replace(/[\u0000-\u001f\u007f]/g, "").trim().slice(0, 48)
    : "";
  const candidateRevision = typeof value.revision === "string"
    ? value.revision.trim().toLowerCase()
    : "";
  const revision = /^[0-9a-f]{7,12}$/.test(candidateRevision) ? candidateRevision : "-------";
  return {
    path,
    title: typeof value.title === "string" && value.title.trim() ? value.title : fallbackTitle,
    branch: branch || "BRANCH UNKNOWN",
    revision,
    color: typeof value.color === "string" && /^#[0-9a-f]{6}$/i.test(value.color)
      ? value.color
      : "#6a6fd1",
  };
}

export function normalizeCartridges(values, pinnedPath = null, limit = MAX_CARTRIDGES) {
  const unique = [];
  const seen = new Set();
  for (const value of Array.isArray(values) ? values : []) {
    const metadata = cartridgeMetadata(value);
    if (!metadata || seen.has(metadata.path)) continue;
    seen.add(metadata.path);
    unique.push(metadata);
  }
  const pinnedIndex = unique.findIndex(({ path }) => path === pinnedPath);
  if (pinnedIndex > 0) unique.unshift(unique.splice(pinnedIndex, 1)[0]);
  return unique.slice(0, Math.max(0, limit));
}

export function upsertCartridge(values, value, limit = MAX_CARTRIDGES) {
  const items = normalizeCartridges(values, null, limit);
  const metadata = cartridgeMetadata(value);
  if (!metadata) return { items, accepted: false };
  const index = items.findIndex(({ path }) => path === metadata.path);
  if (index >= 0) {
    items[index] = metadata;
    return { items, accepted: true };
  }
  if (items.length >= limit) return { items, accepted: false };
  items.push(metadata);
  return { items, accepted: true };
}

export function cartridgeDragIntent(
  deltaY,
  {
    canLoad = true,
    canRecycle = true,
    threshold = CARTRIDGE_DRAG_THRESHOLD,
  } = {},
) {
  if (!Number.isFinite(deltaY)) return null;
  if (canLoad && deltaY <= -threshold) return "load";
  if (canRecycle && deltaY >= threshold) return "recycle";
  return null;
}
