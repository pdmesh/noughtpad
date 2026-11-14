import { useEffect, useRef, useState, type ReactNode } from "react";

interface Props {
  /** What the popover hangs from: the clickable part of the details line. */
  anchor: ReactNode;
  /** Shown on hover, read-only. Omit for no hover preview. */
  peek?: ReactNode;
  /** Shown when open, for editing. */
  children: ReactNode;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  label: string;
  className?: string;
}

const PEEK_DELAY_MS = 300;

/**
 * A small panel under part of the details line. Hovering only previews (`peek`);
 * a click, or Enter on the anchor, opens it for editing (the anchor's own
 * onClick calls `onOpenChange(true)`). Escape or a click outside closes it.
 */
export function Popover({ anchor, peek, children, open, onOpenChange, label, className = "" }: Props) {
  const wrap = useRef<HTMLSpanElement>(null);
  const [peeking, setPeeking] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    if (!open) return;
    clearTimeout(timer.current);
    setPeeking(false);
    // Close after the click has moved focus, so a field inside gets its blur
    // (a half-typed label is added) before it unmounts.
    const onDown = (e: MouseEvent) => {
      if (!wrap.current?.contains(e.target as Node)) setTimeout(() => onOpenChange(false));
    };
    // Tabbing (or ⌘K) away closes it too.
    const onFocusOut = (e: FocusEvent) => {
      const to = e.relatedTarget as Node | null;
      if (to && !wrap.current?.contains(to)) onOpenChange(false);
    };
    // Only an Escape from inside the popover is ours; others (the ⌘K palette) are left alone.
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || !wrap.current?.contains(e.target as Node)) return;
      e.stopPropagation();
      onOpenChange(false);
      wrap.current?.querySelector<HTMLElement>(".popover-anchor button, .popover-anchor [tabindex]")?.focus();
    };
    const w = wrap.current;
    document.addEventListener("mousedown", onDown, true);
    document.addEventListener("keydown", onKey, true);
    w?.addEventListener("focusout", onFocusOut);
    return () => {
      document.removeEventListener("mousedown", onDown, true);
      document.removeEventListener("keydown", onKey, true);
      w?.removeEventListener("focusout", onFocusOut);
    };
  }, [open, onOpenChange]);

  useEffect(() => () => clearTimeout(timer.current), []);

  const hover = (on: boolean) => {
    clearTimeout(timer.current);
    if (!peek || open) return setPeeking(false);
    timer.current = setTimeout(() => setPeeking(on), on ? PEEK_DELAY_MS : 120);
  };

  return (
    <span ref={wrap} className={`popover-wrap ${className}`} onMouseEnter={() => hover(true)} onMouseLeave={() => hover(false)}>
      <span className="popover-anchor">{anchor}</span>
      {open ? (
        <div className="popover" role="dialog" aria-label={label}>
          {children}
        </div>
      ) : (
        peeking && (
          <div className="popover is-peek" aria-hidden>
            {peek}
          </div>
        )
      )}
    </span>
  );
}
