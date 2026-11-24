// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Fake mouse cursor for the scripted marketing demo.
 *
 * A fixed, pointer-events:none overlay glides between elements via the Web
 * Animations API. Falls back to a plain timer where `Element.animate` is
 * missing (jsdom), so the script timing stays testable.
 */

/** Windows arrow (white fill, black outline: the stock cursor on a dark UI). Tip at the origin. */
const CURSOR_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="15" height="22" viewBox="0 0 14 21"
  style="filter:drop-shadow(1px 2px 2px rgba(0,0,0,.45))"><path d="M0.6 0.6V16.6L4.5 12.9L7.2 19.2L9.6 18.1L6.9 11.9H12.3Z"
  fill="#fff" stroke="#000" stroke-width="1" stroke-linejoin="miter"/></svg>`;

/** Windows link-select hand shown over buttons; margin puts the index fingertip on the origin. */
const HAND_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="19" height="25" viewBox="0 0 17 22"
  style="margin:-1px 0 0 -7px;filter:drop-shadow(1px 2px 2px rgba(0,0,0,.45))"><path d="M5 10.8V2.4Q5 1 6.3 1T7.6 2.4V7.2Q7.6 6.4 8.9 6.4T10.2 7.6Q10.2 7 11.5 7T12.8 8.2Q12.8 7.9 14 7.9T15.3 9.2V14.5Q15.3 17 13.5 18.5V21H6.5V18.5L2.2 12.6Q1.5 11.3 2.7 10.7Q3.8 10.3 5 11.8Z"
  fill="#fff" stroke="#000" stroke-width="0.9" stroke-linejoin="round"/></svg>`;

/** macOS arrow (black fill, white outline: the system cursor in light and dark mode). Tip at the origin. */
const MAC_CURSOR_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="17" height="25" viewBox="0 0 15 22"
  style="margin:-1px 0 0 -1px;filter:drop-shadow(0 1px 1.5px rgba(0,0,0,.5))"><path d="M1.5 1.2V17.6L5.4 13.9L8.1 20.2L10.9 19L8.2 12.8H13.6Z"
  fill="#000" stroke="#fff" stroke-width="1.4" stroke-linejoin="round"/></svg>`;

/** macOS pointing hand (black fill, white outline); margin puts the index fingertip on the origin. */
const MAC_HAND_SVG = HAND_SVG.replace('fill="#fff" stroke="#000" stroke-width="0.9"', 'fill="#000" stroke="#fff" stroke-width="1.2"');

/** Match the OS the demo is recorded on, so the cursor looks like the real one. */
const IS_MAC = typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform || navigator.userAgent);
const ARROW = IS_MAC ? MAC_CURSOR_SVG : CURSOR_SVG;
const HAND = IS_MAC ? MAC_HAND_SVG : HAND_SVG;

const Z_INDEX = '2147483647';
const MOVE_MS = 1100;
const CLICK_MS = 450;
const FADE_MS = 400;

/** Offset from an element's center, as a fraction of its width/height (0.25 = a quarter over). */
export interface CursorOffset {
  x: number;
  y: number;
}

/** Handle for driving the fake cursor. */
export interface DemoCursor {
  /** Glide the cursor tip to `el`, `offset` (fractions of its size) away from dead center. */
  moveTo: (el: Element, offset?: CursorOffset) => Promise<void>;
  /** Play the press + ripple animation at the current position. */
  click: () => Promise<void>;
  /** Fade the cursor out and remove it from the DOM. */
  fadeOutAndRemove: () => Promise<void>;
  /** Remove the cursor immediately (cancel path). */
  dispose: () => void;
}

/** Run a WAAPI animation, or wait out its duration when WAAPI is unavailable. */
async function play(
  el: HTMLElement,
  keyframes: Keyframe[],
  options: KeyframeAnimationOptions
): Promise<void> {
  if (typeof el.animate !== 'function') {
    await new Promise<void>(resolve => setTimeout(resolve, Number(options.duration ?? 0)));
    return;
  }
  const anim: Animation = el.animate(keyframes, options);
  try {
    await anim.finished;
  } catch {
    // Cancelled mid-flight (dispose) - nothing to do.
  }
}

/** Create the cursor overlay, starting near the lower right of the viewport. */
export function createDemoCursor(): DemoCursor {
  const cursor: HTMLDivElement = document.createElement('div');
  cursor.setAttribute('aria-hidden', 'true');
  cursor.style.cssText = `position:fixed;left:0;top:0;pointer-events:none;z-index:${Z_INDEX};will-change:transform;`;
  cursor.innerHTML = ARROW;
  let isHand = false;
  /** Real cursors turn into a pointing hand over buttons; mirror that. */
  const setHand = (hand: boolean): void => {
    if (hand === isHand) return;
    isHand = hand;
    cursor.innerHTML = hand ? HAND : ARROW;
  };

  const ripple: HTMLDivElement = document.createElement('div');
  ripple.setAttribute('aria-hidden', 'true');
  ripple.style.cssText = `position:fixed;left:0;top:0;width:44px;height:44px;margin:-22px 0 0 -22px;border-radius:50%;
    border:2px solid rgba(255,255,255,.85);background:rgba(255,255,255,.18);opacity:0;pointer-events:none;z-index:${Z_INDEX};`;

  document.body.append(ripple, cursor);
  // Top layer beats every z-index and WebKitGTK's backdrop-filter compositing quirks
  // (the record button's blur was painting over the cursor). Reset popover UA styles.
  for (const el of [ripple, cursor]) {
    if (typeof el.showPopover !== 'function') continue;
    el.setAttribute('popover', 'manual');
    el.style.border = '0';
    el.style.background = el.style.background || 'transparent';
    el.style.overflow = 'visible';
    el.style.right = 'auto';
    el.style.bottom = 'auto';
    if (el === cursor) el.style.margin = '0';
    el.style.padding = '0';
    try {
      el.showPopover();
    } catch {
      // Not attached / unsupported - plain fixed positioning still works.
    }
  }

  let x: number = window.innerWidth * 0.8;
  let y: number = window.innerHeight * 0.75;
  const place = (px: number, py: number): void => {
    x = px;
    y = py;
    cursor.style.transform = `translate(${px}px, ${py}px)`;
  };
  place(x, y);

  return {
    async moveTo(el: Element, offset: CursorOffset = { x: 0, y: 0 }): Promise<void> {
      const rect: DOMRect = el.getBoundingClientRect();
      const tx: number = rect.left + rect.width * (0.5 + offset.x);
      const ty: number = rect.top + rect.height * (0.5 + offset.y);
      // Slight bow perpendicular to the path so it reads as a human hand, not a laser.
      const dx: number = tx - x;
      const dy: number = ty - y;
      const bow: number = Math.hypot(dx, dy) * 0.08;
      const mx: number = x + dx / 2 - (dy / (Math.hypot(dx, dy) || 1)) * bow;
      const my: number = y + dy / 2 + (dx / (Math.hypot(dx, dy) || 1)) * bow;
      await play(
        cursor,
        [
          { transform: `translate(${x}px, ${y}px)` },
          { transform: `translate(${mx}px, ${my}px)`, offset: 0.5 },
          { transform: `translate(${tx}px, ${ty}px)` },
        ],
        { duration: MOVE_MS, easing: 'cubic-bezier(0.45, 0, 0.2, 1)' }
      );
      place(tx, ty);
      setHand(el.closest('button') !== null);
    },

    async click(): Promise<void> {
      ripple.style.left = `${x}px`;
      ripple.style.top = `${y}px`;
      await Promise.all([
        play(
          ripple,
          [
            { transform: 'scale(0.3)', opacity: 0.9 },
            { transform: 'scale(1.5)', opacity: 0 },
          ],
          { duration: CLICK_MS, easing: 'ease-out' }
        ),
        play(
          cursor,
          [
            { transform: `translate(${x}px, ${y}px) scale(1)` },
            { transform: `translate(${x}px, ${y}px) scale(0.82)`, offset: 0.35 },
            { transform: `translate(${x}px, ${y}px) scale(1)` },
          ],
          { duration: CLICK_MS }
        ),
      ]);
    },

    async fadeOutAndRemove(): Promise<void> {
      await play(cursor, [{ opacity: 1 }, { opacity: 0 }], { duration: FADE_MS });
      cursor.remove();
      ripple.remove();
    },

    dispose(): void {
      cursor.getAnimations?.().forEach(a => a.cancel());
      cursor.remove();
      ripple.remove();
    },
  };
}
