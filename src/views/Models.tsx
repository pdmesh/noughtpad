import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Cloud, HardDrive, KeyRound, Loader2, Plus, Trash2, X } from "lucide-react";
import { useEvent } from "../lib/useEvent";
import { api, isApiError, type DownloadEvent, type LlmOverview, type LocalEntry, type RemoteKind } from "../lib/api";

const GB = 1e9;
const gb = (b: number) => `${(b / GB).toFixed(1)} GB`;
const message = (e: unknown) => (isApiError(e) ? e.message : String(e));

export function useLlm() {
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ["llm"], queryFn: api.llmOverview });
  useEvent<DownloadEvent>("llm-download", (e) => {
    qc.setQueryData<LlmOverview>(
      ["llm"],
      (o) =>
        o && {
          ...o,
          local: o.local.map((l) =>
            l.model.id === e.id
              ? {
                  ...l,
                  received: e.received,
                  downloading: e.state === "downloading",
                  downloaded: e.state === "done" || l.downloaded,
                }
              : l,
          ),
        },
    );
    if (e.state !== "downloading") void qc.invalidateQueries({ queryKey: ["llm"] });
  });
  return q;
}

/**
 * The Models page: the one place a language model is switched on. Two
 * explicit paths, a model on this computer or the user's own provider; nothing
 * runs until one is chosen with "Use".
 */
export function ModelsView({ focus }: { focus?: "local" | "remote" }) {
  const qc = useQueryClient();
  const { data } = useLlm();
  const [error, setError] = useState<string | null>(null);
  const refresh = () => qc.invalidateQueries({ queryKey: ["llm"] });
  const act = (fn: () => Promise<unknown>) => async () => {
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(message(e));
    }
    void refresh();
  };

  useEffect(() => {
    if (focus) document.getElementById(`models-${focus}`)?.scrollIntoView({ block: "start" });
  }, [focus, !!data]);

  if (!data) return null;
  const activeLocal = data.active?.kind === "local" ? data.active.model : null;
  const activeRemote = data.active?.kind === "remote" ? data.active.id : null;

  return (
    <div className="models">
      <h2 className="journal-heading">Models</h2>
      <p className="ideas-lede">
        Noughtpad can think with your notes using a language model. It's off until you choose one here, and every answer shows which notes it read.
      </p>

      <div className={`model-status ${data.active ? "is-on" : ""}`} role="status">
        {data.active ? (
          <>
            <span>
              Answering with <strong>{data.activeLabel}</strong>.{" "}
              {activeLocal ? "Your notes stay on this computer." : "The notes you choose are sent to this provider when you ask."}
            </span>
            <button className="btn btn-small" onClick={act(api.llmTurnOff)}>
              Turn off
            </button>
          </>
        ) : (
          <span>Off. Nothing is sent anywhere. Choose a model below to start.</span>
        )}
      </div>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}

      <section className="ideas-section" id="models-local">
        <h3 className="ideas-section-title">
          <HardDrive size={15} aria-hidden /> On this computer
        </h3>
        <p className="ideas-hint faint">Private and free: runs offline, and your notes never leave this machine. Download once, then choose it.</p>
        {!data.localAvailable && <p className="faint">This build of Noughtpad doesn't include the local runtime.</p>}
        <ul className="model-list">
          {data.local.map((l) => (
            <LocalRow
              key={l.model.id}
              entry={l}
              inUse={activeLocal === l.model.id}
              disabled={!data.localAvailable}
              onDownload={act(() => api.llmDownload(l.model.id))}
              onCancel={act(() => api.llmCancelDownload(l.model.id))}
              onUse={act(() => api.llmUse({ kind: "local", model: l.model.id }))}
              onDelete={act(() => api.llmDeleteModel(l.model.id))}
            />
          ))}
        </ul>
      </section>

      <section className="ideas-section" id="models-remote">
        <h3 className="ideas-section-title">
          <Cloud size={15} aria-hidden /> With your own key
        </h3>
        <p className="ideas-hint faint">
          Use a provider you already pay for, or a model server you run (Ollama, LM Studio). Before anything is sent, Noughtpad shows which notes go where.
        </p>
        {data.remotes.length > 0 && (
          <ul className="model-list">
            {data.remotes.map(({ remote: r, hasKey }) => (
              <li key={r.id} className={`model-row ${activeRemote === r.id ? "is-active" : ""}`}>
                <div className="model-main">
                  <div className="model-name">
                    {r.label} <span className="faint">· {r.model}</span>
                  </div>
                  <div className="model-meta faint">
                    {r.baseUrl ?? data.providers.find((p) => p.kind === r.kind)?.defaultUrl ?? data.providers.find((p) => p.kind === r.kind)?.label}
                    {hasKey && (
                      <>
                        {" · "}
                        <KeyRound size={11} aria-hidden /> key saved
                      </>
                    )}
                  </div>
                </div>
                <div className="model-actions">
                  {activeRemote === r.id ? (
                    <span className="model-inuse">
                      <Check size={13} aria-hidden /> In use
                    </span>
                  ) : (
                    <button className="btn btn-small" onClick={act(() => api.llmUse({ kind: "remote", id: r.id }))}>
                      Use
                    </button>
                  )}
                  <button
                    className="icon-btn"
                    title="Remove provider and its key"
                    aria-label={`Remove ${r.label}`}
                    onClick={act(() => api.llmRemoveRemote(r.id))}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
        {data.remoteAvailable ? <AddProvider data={data} onSaved={refresh} /> : <p className="faint">This build of Noughtpad can't use online providers.</p>}
      </section>
    </div>
  );
}

function LocalRow({
  entry: l,
  inUse,
  disabled,
  onDownload,
  onCancel,
  onUse,
  onDelete,
}: {
  entry: LocalEntry;
  inUse: boolean;
  disabled: boolean;
  onDownload: () => void;
  onCancel: () => void;
  onUse: () => void;
  onDelete: () => void;
}) {
  const m = l.model;
  const pct = Math.round((l.received / m.bytes) * 100);
  return (
    <li className={`model-row ${inUse ? "is-active" : ""}`}>
      <div className="model-main">
        <div className="model-name">
          {m.name} {m.recommended && <span className="chip chip-small">Recommended</span>}
        </div>
        <div className="model-blurb">{m.blurb}</div>
        <div className="model-meta faint">
          {gb(m.bytes)} download · needs about {m.ramGb} GB of memory · {m.maker} · {m.licence}
        </div>
        {l.downloading && (
          <div className="model-progress" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100} aria-label={`Downloading ${m.name}`}>
            <span style={{ width: `${pct}%` }} />
          </div>
        )}
      </div>
      <div className="model-actions">
        {l.downloading ? (
          <>
            <span className="faint model-pct">
              {gb(l.received)} of {gb(m.bytes)}
            </span>
            <button className="icon-btn" onClick={onCancel} title="Pause download" aria-label={`Pause downloading ${m.name}`}>
              <X size={14} />
            </button>
          </>
        ) : l.downloaded ? (
          <>
            {inUse ? (
              <span className="model-inuse">
                <Check size={13} aria-hidden /> In use
              </span>
            ) : (
              <button className="btn btn-small btn-primary" onClick={onUse} disabled={disabled}>
                Use
              </button>
            )}
            <button className="icon-btn" title="Delete the downloaded model" aria-label={`Delete ${m.name}`} onClick={onDelete}>
              <Trash2 size={14} />
            </button>
          </>
        ) : (
          <button className="btn btn-small" onClick={onDownload} disabled={disabled}>
            {l.received > 0 ? `Resume (${gb(l.received)} of ${gb(m.bytes)})` : "Download"}
          </button>
        )}
      </div>
    </li>
  );
}

function AddProvider({ data, onSaved }: { data: LlmOverview; onSaved: () => void }) {
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState<RemoteKind>("anthropic");
  const [key, setKey] = useState("");
  const [url, setUrl] = useState("");
  const [model, setModel] = useState("");
  const [models, setModels] = useState<string[] | null>(null);
  const [busy, setBusy] = useState<"models" | "save" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const p = data.providers.find((x) => x.kind === kind)!;

  const reset = (k: RemoteKind) => {
    setKind(k);
    setKey("");
    setUrl("");
    setModel("");
    setModels(null);
    setError(null);
  };
  const loadModels = async () => {
    setBusy("models");
    setError(null);
    try {
      const names = await api.llmListRemoteModels(kind, url || undefined, key || undefined);
      setModels(names);
      if (!model && names.length) setModel(names[0]);
    } catch (e) {
      setError(message(e));
    }
    setBusy(null);
  };
  const save = async () => {
    setBusy("save");
    setError(null);
    try {
      await api.llmSaveRemote({
        kind,
        baseUrl: url || undefined,
        model,
        apiKey: key || undefined,
      });
      // Saving is not opting in: the user still presses "Use".
      onSaved();
      setOpen(false);
      reset(kind);
    } catch (e) {
      setError(message(e));
    }
    setBusy(null);
  };

  if (!open) {
    return (
      <button className="btn btn-small" onClick={() => setOpen(true)}>
        <Plus size={14} aria-hidden /> Add a provider
      </button>
    );
  }
  const canList = (!p.needsKey || key.trim()) && (!p.needsUrl || url.trim());
  return (
    <form
      className="provider-form"
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <div className="segmented" role="radiogroup" aria-label="Provider">
        {data.providers.map((x) => (
          <button
            type="button"
            key={x.kind}
            role="radio"
            aria-checked={x.kind === kind}
            className={x.kind === kind ? "is-on" : ""}
            onClick={() => reset(x.kind)}
          >
            {x.label}
          </button>
        ))}
      </div>
      {p.needsKey && (
        <label className="field">
          <span>API key</span>
          <input type="password" autoComplete="off" spellCheck={false} value={key} onChange={(e) => setKey(e.target.value)} placeholder="Paste your key" />
          <small className="faint">
            {data.keychain
              ? "Stored in your system keychain, never in your notes folder."
              : "No system keychain found: the key is kept only until Noughtpad quits."}
          </small>
        </label>
      )}
      {(p.needsUrl || p.defaultUrl) && (
        <label className="field">
          <span>Server address</span>
          <input type="url" spellCheck={false} value={url} onChange={(e) => setUrl(e.target.value)} placeholder={p.defaultUrl ?? "https://…/v1"} />
        </label>
      )}
      {!p.needsKey && p.needsUrl && (
        <label className="field">
          <span>API key (optional)</span>
          <input type="password" autoComplete="off" spellCheck={false} value={key} onChange={(e) => setKey(e.target.value)} />
        </label>
      )}
      <label className="field">
        <span>Model</span>
        <div className="field-row">
          <input
            list="provider-models"
            spellCheck={false}
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder={models ? "Choose a model" : "Model name"}
          />
          <datalist id="provider-models">
            {models?.map((m) => (
              <option key={m} value={m} />
            ))}
          </datalist>
          <button type="button" className="btn btn-small" onClick={() => void loadModels()} disabled={!canList || busy !== null}>
            {busy === "models" ? <Loader2 size={13} className="spin" aria-hidden /> : null} Load models
          </button>
        </div>
        {models && (
          <small className="faint">
            {models.length} {models.length === 1 ? "model" : "models"} available. Type to filter.
          </small>
        )}
      </label>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="field-row">
        <button type="submit" className="btn btn-small btn-primary" disabled={!model.trim() || busy !== null}>
          Save provider
        </button>
        <button type="button" className="btn btn-small" onClick={() => setOpen(false)}>
          Cancel
        </button>
      </div>
    </form>
  );
}
