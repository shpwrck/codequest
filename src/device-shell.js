export const DEVICE_MESSAGE_MAX_LENGTH = 160;
export const GUIDE_MESSAGE_MAX_LENGTH = 48;

const CONTROL_ACTIVATION_CODES = new Set(["Enter", "NumpadEnter"]);

export function deviceMessageText(
  value,
  {
    fallback = "UNKNOWN FAULT",
    maxLength = DEVICE_MESSAGE_MAX_LENGTH,
  } = {},
) {
  const raw = value instanceof Error ? value.message : value?.message ?? value;
  const text = String(raw ?? "").replace(/\s+/g, " ").trim();
  if (!text) return fallback;
  return text.length > maxLength ? `${text.slice(0, maxLength - 1).trimEnd()}…` : text;
}

export function leavesKeyToFocusedControl(code, focusedControl) {
  return Boolean(focusedControl) && CONTROL_ACTIVATION_CODES.has(code);
}

export function trappedFocusTarget(focusables, active, backwards = false) {
  if (!focusables.length) return null;
  const index = focusables.indexOf(active);
  if (index < 0) return backwards ? focusables.at(-1) : focusables[0];
  if (backwards && index === 0) return focusables.at(-1);
  if (!backwards && index === focusables.length - 1) return focusables[0];
  return null;
}
