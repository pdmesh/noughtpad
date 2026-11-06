/**
 * A touch-primary device (phone, tablet): no hover, no right-click. Checked
 * once, since a device's pointer type doesn't change mid-session; used to
 * swap hover-only UI (widget tools, the right-click menu) for tap-friendly
 * alternatives (see docs/MOBILE.md § Phone layout).
 */
export const isCoarsePointer = typeof window !== "undefined" && !!window.matchMedia?.("(pointer: coarse)").matches;
