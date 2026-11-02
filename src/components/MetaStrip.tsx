import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, Link2, Tag, X } from "lucide-react";
import { api, type EditableMeta } from "../lib/api";
import { LabelInput } from "./LabelInput";

interface Props {
  meta: EditableMeta;
  onChange: (m: EditableMeta) => void;
}

/**
 * Optional source + labels. Invisible until hovered/focused (or faint when
 * filled), so the blank page stays blank.
 */
export function MetaStrip({ meta, onChange }: Props) {
  const [showSource, setShowSource] = useState(false);
  const { data: allLabels = [] } = useQuery({ queryKey: ["notes", "labels"], queryFn: api.labels });

  const hasSource = !!(meta.source?.title || meta.source?.url);
  const filled = meta.labels.length > 0 || hasSource;
  const sourceOpen = showSource || hasSource;

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
        </div>
      )}
    </div>
  );
}
