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

/** Wheel travel, in CSS pixels, that turns the volume wheel one detent. A
 * classic notched mouse reports about this much per notch. */
export const WHEEL_DETENT_PX = 100;
/** A pause longer than this ends a wheel gesture. */
export const WHEEL_GESTURE_GAP_MS = 200;
// WheelEvent.deltaMode: DOM_DELTA_PIXEL, DOM_DELTA_LINE, DOM_DELTA_PAGE.
const WHEEL_UNIT_PX = Object.freeze([1, 40, WHEEL_DETENT_PX]);

/* Precision touchpads and smooth-scrolling mice send dozens of small wheel
 * events per gesture, so the wheel moves by travel, not by event count. The
 * first movement of a gesture clicks one detent at once (a light nudge still
 * responds), and every further WHEEL_DETENT_PX of travel in the same
 * direction clicks one more. The returned roll takes (deltaY, deltaMode, now)
 * and answers how many detents to turn: positive rolls up (louder). */
export function createWheelDetents({
  detent = WHEEL_DETENT_PX,
  gapMs = WHEEL_GESTURE_GAP_MS,
} = {}) {
  let travel = 0;
  let clicked = 0;
  let lastAt = -Infinity;
  return function roll(deltaY, deltaMode, now) {
    const pixels = Number(deltaY) * (WHEEL_UNIT_PX[deltaMode] ?? 1);
    if (!Number.isFinite(pixels) || pixels === 0) return 0;
    if (now - lastAt > gapMs || Math.sign(pixels) !== Math.sign(travel)) {
      travel = 0;
      clicked = 0;
    }
    lastAt = now;
    travel += pixels;
    const due = Math.max(1, Math.floor(Math.abs(travel) / detent));
    const detents = due - clicked;
    clicked = due;
    return detents === 0 ? 0 : -Math.sign(travel) * detents;
  };
}

export function trappedFocusTarget(focusables, active, backwards = false) {
  if (!focusables.length) return null;
  const index = focusables.indexOf(active);
  if (index < 0) return backwards ? focusables.at(-1) : focusables[0];
  if (backwards && index === 0) return focusables.at(-1);
  if (!backwards && index === focusables.length - 1) return focusables[0];
  return null;
}
