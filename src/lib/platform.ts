/**
 * A touch-primary device (phone, tablet): no hover, no right-click. Checked
 * once, since a device's pointer type doesn't change mid-session; used to
 * swap hover-only UI (widget tools, the right-click menu) for tap-friendly
 * alternatives (see docs/MOBILE.md § Phone layout).
 */
export const isCoarsePointer = typeof window !== "undefined" && !!window.matchMedia?.("(pointer: coarse)").matches;

/** How the UI names the machine it runs on: "this computer", or on a phone or tablet "this device". */
export const thisDevice = isCoarsePointer ? "this device" : "this computer";
