import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, ExternalLink, Link2, Tag, X } from "lucide-react";
import { api, type EditableMeta } from "../lib/api";
import { LabelInput } from "./LabelInput";

interface Props {
  meta: EditableMeta;
  onChange: (m: EditableMeta) => void;
  onOpenUrl: (url: string) => void;
}

/**
 * Optional labels + source, on one quiet line under the title. Invisible until
 * hovered/focused when empty, faint when filled, so the page stays calm. The
 * source shows its name only; its URL field opens while the source is being
 * edited, and the ↗ button (with the full URL as its tooltip) opens the link.
 */
export function MetaStrip({ meta, onChange, onOpenUrl }: Props) {
  const [showSource, setShowSource] = useState(false);
  const { data: allLabels = [] } = useQuery({ queryKey: ["notes", "labels"], queryFn: api.labels });

  const hasSource = !!(meta.source?.title || meta.source?.url);
  const filled = meta.labels.length > 0 || hasSource;
  const sourceOpen = showSource || hasSource;
  const url = meta.source?.url?.trim() ?? "";
  const href = openableUrl(url);

  const setSource = (field: "title" | "url", value: string) => {
    const source = { ...(meta.source ?? {}), [field]: value };
    onChange({ ...meta, source });
  };

  return (
    <div className={`meta-strip ${filled ? "meta-filled" : "meta-empty"}`}>
      <div className="meta-row">
        <Tag size={13} className="meta-icon" aria-hidden />
        {meta.labels.map((l) => (
          <span key={l} className="chip">
            {l}
            <button
              className="chip-x"
              aria-label={`Remove label ${l}`}
              onClick={() => onChange({ ...meta, labels: meta.labels.filter((x) => x !== l) })}
            >
              <X size={11} />
            </button>
          </span>
        ))}
        <LabelInput
          current={meta.labels}
          all={allLabels}
          onAdd={(labels) => onChange({ ...meta, labels: [...meta.labels, ...labels] })}
          onRemoveLast={() => onChange({ ...meta, labels: meta.labels.slice(0, -1) })}
        />
        {sourceOpen ? (
          <span className="meta-source">
            <BookOpen size={13} className="meta-icon" aria-hidden />
            <input
              className="meta-input meta-source-title"
              value={meta.source?.title ?? ""}
              // With only a URL, its site stands in for the name, so there's still something to click.
              placeholder={hostname(url) ?? "Source (book, article, video…)"}
              aria-label="Source title"
              title={meta.source?.title || undefined}
              size={Math.max(8, (meta.source?.title || hostname(url) || "Source (book, article, video…)").length)}
              autoFocus={showSource && !hasSource}
              onChange={(e) => setSource("title", e.target.value)}
            />
            <Link2 size={13} className="meta-icon meta-source-url-icon" aria-hidden />
            <input
              className="meta-input meta-source-url"
              value={meta.source?.url ?? ""}
              placeholder="URL"
              aria-label="Source URL"
              title={url || undefined}
              onChange={(e) => setSource("url", e.target.value)}
            />
            {href && (
              <button
                className="icon-btn meta-open"
                title={`Open in your browser: ${href}`}
                aria-label="Open the source URL in your browser"
                onClick={() => onOpenUrl(href)}
              >
                <ExternalLink size={13} />
              </button>
            )}
          </span>
        ) : (
          <button className="meta-add" onClick={() => setShowSource(true)}>
            <BookOpen size={13} aria-hidden /> Add source
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
