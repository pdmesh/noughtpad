import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, ExternalLink, Heading, Plus, Tag, X } from "lucide-react";
import { api, type EditableMeta } from "../lib/api";
import { fitCount } from "../lib/metaLayout";
import { LabelInput } from "./LabelInput";
import { Popover } from "./Popover";

interface Props {
  meta: EditableMeta;
  onChange: (m: EditableMeta) => void;
  onOpenUrl: (url: string) => void;
  /** Offered (as "+ Title" on hover) when the note has no title. */
  onAddTitle?: () => void;
}

const CHIP_GAP = 4;
const MORE_CHIP_W = 34;

/**
 * Labels and source on one line that never wraps. As many labels as fit are
 * shown, then "+N"; the rest, and editing, live in popovers (hover previews,
 * click edits). The source shows its name; its URL is in a popover and behind ↗.
 * Invisible until hovered when empty, faint when filled, so the page stays calm.
 */
export function MetaStrip({ meta, onChange, onOpenUrl, onAddTitle }: Props) {
  const [tagsOpen, setTagsOpen] = useState(false);
  const [sourceOpen, setSourceOpen] = useState(false);
  const { data: allLabels = [] } = useQuery({ queryKey: ["notes", "labels"], queryFn: api.labels });

  const labels = meta.labels;
  const hasSource = !!(meta.source?.title || meta.source?.url);
  const filled = labels.length > 0 || hasSource;
  const url = meta.source?.url?.trim() ?? "";
  const href = openableUrl(url);
  const sourceName = meta.source?.title?.trim() || hostname(url) || "";

  const setSource = (field: "title" | "url", value: string) => onChange({ ...meta, source: { ...(meta.source ?? {}), [field]: value } });
  const removeLabel = (l: string) => onChange({ ...meta, labels: labels.filter((x) => x !== l) });

  // How many labels fit: measured from an invisible copy of all the chips,
  // against the room the rest of the line leaves.
  const row = useRef<HTMLDivElement>(null);
  const tags = useRef<HTMLSpanElement>(null);
  const measure = useRef<HTMLSpanElement>(null);
  const [shown, setShown] = useState(labels.length);
  const refit = useCallback(() => {
    const r = row.current;
    const t = tags.current;
    const m = measure.current;
    if (!r || !t || !m) return;
    const own = t.closest(".popover-wrap");
    const others = Array.from(r.children).reduce((sum, c) => (c === own || c === m ? sum : sum + (c as HTMLElement).offsetWidth + 6), 0);
    const widths = Array.from(m.children).map((c) => (c as HTMLElement).offsetWidth);
    // 26px: the tag icon, its gap and the button's padding, in front of the chips.
    setShown(fitCount(widths, r.clientWidth - others - 26, CHIP_GAP, MORE_CHIP_W));
  }, []);
  // Anything else on the line changing size changes the room left for labels.
  useLayoutEffect(refit, [refit, labels, sourceName, href, onAddTitle]);
  useLayoutEffect(() => {
    const ro = new ResizeObserver(refit);
    if (row.current) ro.observe(row.current);
    void document.fonts?.ready.then(refit);
    return () => ro.disconnect();
  }, [refit]);

  const hidden = labels.length - shown;
  const chips = (list: string[], removable: boolean) =>
    list.map((l) => (
      <span key={l} className="chip">
        {l}
        {removable && (
          <button className="chip-x" aria-label={`Remove label ${l}`} onClick={() => removeLabel(l)}>
            <X size={11} />
          </button>
        )}
      </span>
    ));

  return (
    <div className={`meta-strip ${filled ? "meta-filled" : "meta-empty"}`}>
      <div className="meta-row" ref={row}>
        <span className="meta-measure" ref={measure} aria-hidden>
          {chips(labels, false)}
        </span>
        <Popover
          className="meta-tags"
          label="Labels"
          open={tagsOpen}
          onOpenChange={setTagsOpen}
          peek={labels.length > 0 && <span className="popover-chips">{chips(labels, false)}</span>}
          anchor={
            <span className="meta-tags-line" ref={tags}>
              <button className="meta-tag-btn" onClick={() => setTagsOpen(true)} title="Labels" aria-label={labels.length ? `Labels: ${labels.join(", ")}` : "Add label"}>
                <Tag size={13} className="meta-icon" aria-hidden />
                {chips(labels.slice(0, shown), false)}
                {hidden > 0 && <span className="chip chip-more">+{hidden}</span>}
                {labels.length === 0 && <Plus size={12} className="meta-hover-only" aria-hidden />}
              </button>
            </span>
          }
        >
          {labels.length > 0 && <div className="popover-chips">{chips(labels, true)}</div>}
          <LabelInput
            current={labels}
            all={allLabels}
            autoFocus
            onAdd={(add) => onChange({ ...meta, labels: [...labels, ...add] })}
            onRemoveLast={() => onChange({ ...meta, labels: labels.slice(0, -1) })}
          />
        </Popover>

        <Popover
          className="meta-source"
          label="Source"
          open={sourceOpen}
          onOpenChange={setSourceOpen}
          peek={url && <span className="popover-url">{url}</span>}
          anchor={
            hasSource ? (
              <button className="meta-source-btn" onClick={() => setSourceOpen(true)} title="Edit source">
                <BookOpen size={13} className="meta-icon" aria-hidden />
                <span className="meta-source-name">{sourceName}</span>
              </button>
            ) : (
              <button className="meta-add meta-hover-only" onClick={() => setSourceOpen(true)}>
                <BookOpen size={13} aria-hidden /> Add source
              </button>
            )
          }
        >
          <label className="popover-field">
            <span>Name</span>
            <input
              className="settings-input"
              value={meta.source?.title ?? ""}
              placeholder={hostname(url) ?? "Book, article, video…"}
              aria-label="Source title"
              spellCheck={false}
              autoCorrect="off"
              autoFocus
              onChange={(e) => setSource("title", e.target.value)}
            />
          </label>
          <label className="popover-field">
            <span>URL</span>
            <input className="settings-input" value={meta.source?.url ?? ""} placeholder="https://…" aria-label="Source URL" spellCheck={false} autoCorrect="off" autoCapitalize="off" onChange={(e) => setSource("url", e.target.value)} />
          </label>
          {hasSource && (
            <div className="popover-footer">
              {href ? (
                <button className="popover-link" onClick={() => onOpenUrl(href)}>
                  Open link <ExternalLink size={12} aria-hidden />
                </button>
              ) : (
                <span />
              )}
              <button
                className="popover-link popover-danger"
                onClick={() => {
                  onChange({ ...meta, source: null });
                  setSourceOpen(false);
                }}
              >
                Remove source
              </button>
            </div>
          )}
        </Popover>
        {href && (
          <button className="icon-btn meta-open" title={`Open in your browser: ${href}`} aria-label="Open the source URL in your browser" onClick={() => onOpenUrl(href)}>
            <ExternalLink size={13} />
          </button>
        )}
        {onAddTitle && (
          <button className="meta-add meta-hover-only" onClick={onAddTitle} title="Add a title">
            <Heading size={13} aria-hidden /> Title
          </button>
        )}
      </div>
    </div>
  );
}

function hostname(url: string): string | null {
  const href = openableUrl(url);
  return href ? new URL(href).hostname.replace(/^www\./, "") : null;
}

/** A source URL worth offering to open: http(s), or a bare domain such as `example.com/page`. */
export function openableUrl(raw: string): string | null {
  const s = raw.trim();
  if (!s || /\s/.test(s)) return null;
  const withScheme = /^[a-z][a-z0-9+.-]*:/i.test(s) ? s : `https://${s}`;
  try {
    const u = new URL(withScheme);
    if (u.protocol !== "http:" && u.protocol !== "https:") return null;
    if (!u.hostname.includes(".") && u.hostname !== "localhost") return null;
    return u.href;
  } catch {
    return null;
  }
}
