import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, Link2, Tag, X } from "lucide-react";
import { api, type EditableMeta } from "../lib/api";

interface Props {
  meta: EditableMeta;
  onChange: (m: EditableMeta) => void;
}

/**
 * Optional source + labels. Invisible until hovered/focused (or faint when
 * filled), so the blank page stays blank.
 */
export function MetaStrip({ meta, onChange }: Props) {
  const [labelDraft, setLabelDraft] = useState("");
  const [showSource, setShowSource] = useState(false);
  const { data: allLabels = [] } = useQuery({ queryKey: ["notes", "labels"], queryFn: api.labels });

  const hasSource = !!(meta.source?.title || meta.source?.url);
  const filled = meta.labels.length > 0 || hasSource;
  const sourceOpen = showSource || hasSource;

  const addLabel = (raw: string) => {
    const parts = raw.split(",").map((s) => s.trim().replace(/^#/, "")).filter(Boolean);
    if (!parts.length) return;
    const existing = new Set(meta.labels.map((l) => l.toLowerCase()));
    const fresh = parts
      .map((p) => allLabels.find((l) => l.label.toLowerCase() === p.toLowerCase())?.label ?? p)
      .filter((p) => !existing.has(p.toLowerCase()));
    onChange({ ...meta, labels: [...meta.labels, ...fresh] });
    setLabelDraft("");
  };

  const setSource = (field: "title" | "url", value: string) => {
    const source = { ...(meta.source ?? {}), [field]: value };
    onChange({ ...meta, source });
  };

  const suggestions = allLabels
    .filter((l) => !meta.labels.some((m) => m.toLowerCase() === l.label.toLowerCase()))
    .slice(0, 50);

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
        <input
          className="meta-input"
          list="nought-labels"
          value={labelDraft}
          placeholder={meta.labels.length ? "" : "Add label"}
          aria-label="Add label"
          onChange={(e) => {
            const v = e.target.value;
            if (v.endsWith(",")) addLabel(v);
            else setLabelDraft(v);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              addLabel(labelDraft);
            } else if (e.key === "Backspace" && !labelDraft && meta.labels.length) {
              onChange({ ...meta, labels: meta.labels.slice(0, -1) });
            }
          }}
          onBlur={() => labelDraft && addLabel(labelDraft)}
        />
        <datalist id="nought-labels">
          {suggestions.map((l) => (
            <option key={l.label} value={l.label} />
          ))}
        </datalist>
        {!sourceOpen && (
          <button className="meta-add" onClick={() => setShowSource(true)}>
            <BookOpen size={13} aria-hidden /> Add source
          </button>
        )}
      </div>
      {sourceOpen && (
        <div className="meta-row">
          <BookOpen size={13} className="meta-icon" aria-hidden />
          <input
            className="meta-input meta-source-title"
            value={meta.source?.title ?? ""}
            placeholder="Source (book, article, video…)"
            aria-label="Source title"
            autoFocus={showSource && !hasSource}
            onChange={(e) => setSource("title", e.target.value)}
          />
          <Link2 size={13} className="meta-icon" aria-hidden />
          <input
            className="meta-input meta-source-url"
            value={meta.source?.url ?? ""}
            placeholder="URL"
            aria-label="Source URL"
            onChange={(e) => setSource("url", e.target.value)}
          />
          <select
            className="meta-kind"
            aria-label="Note type"
            value={meta.kindOverride ?? "auto"}
            onChange={(e) =>
              onChange({ ...meta, kindOverride: e.target.value === "auto" ? null : (e.target.value as "personal" | "source") })
            }
          >
            <option value="auto">Type: auto</option>
            <option value="personal">Personal</option>
            <option value="source">Source</option>
          </select>
        </div>
      )}
    </div>
  );
}
