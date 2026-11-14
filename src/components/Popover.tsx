import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";

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
    delete wrap.current?.dataset.cancelled;
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
      // Escape discards: fields inside see this on their blur (LabelInput drops its draft).
      if (wrap.current) wrap.current.dataset.cancelled = "1";
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

  // Keep the panel inside the window: one hanging from the right end of the line shifts left.
  const panel = useRef<HTMLDivElement>(null);
  const [shift, setShift] = useState(0);
  const showing = open || peeking;
  useLayoutEffect(() => {
    if (!showing || !panel.current) return setShift(0);
    const r = panel.current.getBoundingClientRect();
    const over = r.right - shift - (window.innerWidth - 12);
    setShift(over > 0 ? over : 0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showing, open]);

  const hover = (on: boolean) => {
    clearTimeout(timer.current);
    if (!peek || open) return setPeeking(false);
    timer.current = setTimeout(() => setPeeking(on), on ? PEEK_DELAY_MS : 120);
  };

  return (
    <span ref={wrap} className={`popover-wrap ${className}`} onMouseEnter={() => hover(true)} onMouseLeave={() => hover(false)}>
      <span className="popover-anchor">{anchor}</span>
      {open ? (
        <div ref={panel} className="popover" role="dialog" aria-label={label} style={{ translate: shift ? `-${shift}px` : undefined }}>
          {children}
        </div>
      ) : (
        peeking && (
          <div ref={panel} className="popover is-peek" aria-hidden style={{ translate: shift ? `-${shift}px` : undefined }}>
            {peek}
          </div>
        )
      )}
    </span>
  );
}
