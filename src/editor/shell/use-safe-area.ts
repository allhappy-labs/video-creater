/**
 * Safe-area padding for phones with a notch, status bar or home indicator. `index.html` sets
 * `viewport-fit=cover`, so the page extends under them and these `env()` insets keep the top bar
 * and bottom tool bar clear. Desktop windows report zero insets, so the classes are always safe.
 */

/** Top bar: pads below the status bar while keeping its own 48 px content height. */
export const safeAreaTopClass = "box-content pt-[env(safe-area-inset-top)]";

/** Bottom tool bar and full-height sheets: pads above the home indicator. */
export const safeAreaBottomClass = "pb-[env(safe-area-inset-bottom)]";
