import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronRight, FolderOpen } from "lucide-react";
import { api, type VaultInfo } from "../lib/api";
import { describeStatus, useSemanticStatus } from "../lib/useSemantic";
import { checkForUpdates, describeUpdate, installUpdate, useUpdateState } from "../lib/updates";
import { useLlm } from "./Models";

const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform);
const mod = isMac ? "⌘" : "Ctrl";

const SHORTCUTS: Array<[string, string]> = [
  [`${mod} N`, "New note"],
  [`${mod} K`, "Search and commands"],
  [`${mod} J`, "Related notes"],
  [`${mod} L`, "New chat"],
  [`${mod} \\`, "Show or hide the sidebar and toolbar"],
  [`${isMac ? "⌘ ⇧" : "Ctrl ⇧"} Space`, "Quick capture from anywhere"],
];

/** One place for preferences that were spread across the palette and sidebar. */
export function SettingsView({
  vault,
  onSwitchVault,
  chromePinned,
  toggleChrome,
  listUnderEditor,
  toggleListUnderEditor,
  onModels,
  onRebuildIndex,
  onResetRelated,
}: {
  vault: VaultInfo;
  onSwitchVault: () => void;
  chromePinned: boolean;
  toggleChrome: () => void;
  listUnderEditor: boolean;
  toggleListUnderEditor: () => void;
  onModels: () => void;
  onRebuildIndex: () => void;
  onResetRelated: () => void;
}) {
  const qc = useQueryClient();
  const semantic = useSemanticStatus();
  const { data: llm } = useLlm();
  const semanticOn = semantic.state !== "off";
  const { data: autostart = false } = useQuery({ queryKey: ["autostart"], queryFn: api.autostartEnabled });
  const semanticLine =
    describeStatus(semantic) ?? (semantic.state === "ready" ? `On · ${semantic.notes.toLocaleString()} notes connected` : "Off");

  return (
    <div className="settings">
      <h2 className="journal-heading">Settings</h2>

      <section className="settings-section">
        <h3 className="ideas-section-title">Notes folder</h3>
        <Row label={vault.path} hint="Plain Markdown files. Noughtpad keeps its index in a .noughtpad folder inside it.">
          <button className="btn btn-small" onClick={onSwitchVault}>
            <FolderOpen size={13} aria-hidden /> Switch…
          </button>
        </Row>
        <Row label="Rebuild the search index" hint="Only needed if search seems out of date. Your notes are not touched.">
          <button className="btn btn-small" onClick={onRebuildIndex}>
            Rebuild
          </button>
        </Row>
      </section>

      <section className="settings-section">
        <h3 className="ideas-section-title">Finding connections</h3>
        <Row
          label="Search by meaning, related notes and ideas over time"
          hint={`Uses a small model that runs on this computer (about 65 MB, downloaded once). ${semanticLine}.`}
        >
          <Toggle
            on={semanticOn}
            label="Search by meaning"
            onChange={() =>
              void (semanticOn ? api.disableSemantic() : api.enableSemantic()).then(() => qc.invalidateQueries({ queryKey: ["semantic-status"] }))
            }
          />
        </Row>
        <Row label="Reset what related notes learned" hint="Related notes adapt to what you open and dismiss. This forgets that and starts fresh.">
          <button className="btn btn-small" onClick={onResetRelated}>
            Reset
          </button>
        </Row>
      </section>

      <section className="settings-section">
        <h3 className="ideas-section-title">Chat</h3>
        <button className="settings-link" onClick={onModels}>
          <span>
            <span className="settings-label">Language models</span>
            <span className="settings-hint faint">{llm?.activeLabel ? `Using ${llm.activeLabel}` : "Off. Nothing is sent anywhere."}</span>
          </span>
          <ChevronRight size={15} aria-hidden />
        </button>
      </section>

      <section className="settings-section">
        <h3 className="ideas-section-title">Layout</h3>
        <Row label="Always show the sidebar and toolbar" hint="Otherwise they stay hidden until you move the pointer to the edge.">
          <Toggle on={chromePinned} label="Always show the sidebar and toolbar" onChange={toggleChrome} />
        </Row>
        <Row label="Show recent notes under the editor" hint="The writing page is a blank page by default.">
          <Toggle on={listUnderEditor} label="Show recent notes under the editor" onChange={toggleListUnderEditor} />
        </Row>
      </section>

      <section className="settings-section">
        <h3 className="ideas-section-title">Start-up</h3>
        <Row
          label="Open at login"
          hint={
            isMac
              ? "Starts in the background, so ⌘ ⇧ Space works straight away. Closing the window keeps Noughtpad running; ⌘ Q quits."
              : "Starts minimised, so Ctrl ⇧ Space works straight away. Closing the window quits Noughtpad."
          }
        >
          <Toggle
            on={autostart}
            label="Open at login"
            onChange={() => void api.setAutostart(!autostart).then((on) => qc.setQueryData(["autostart"], on))}
          />
        </Row>
      </section>

      <Updates />

      <section className="settings-section">
        <h3 className="ideas-section-title">Keyboard</h3>
        <dl className="shortcuts">
          {SHORTCUTS.map(([k, v]) => (
            <div key={k}>
              <dt>
                <kbd className="kbd">{k}</kbd>
              </dt>
              <dd>{v}</dd>
            </div>
          ))}
        </dl>
      </section>
    </div>
  );
}

function Updates() {
  const update = useUpdateState();
  const { data: version } = useQuery({ queryKey: ["app-version"], queryFn: api.appVersion, staleTime: Infinity });
  const busy = update.kind === "checking" || update.kind === "installing";
  const canInstall = update.kind === "available" || (update.kind === "error" && update.version);
  const status = describeUpdate(update);
  return (
    <section className="settings-section">
      <h3 className="ideas-section-title">Updates</h3>
      <Row
        label={version ? `Noughtpad ${version}` : "Noughtpad"}
        hint={status ?? "Noughtpad only looks for updates when you ask. Updates come from GitHub and are checked against Noughtpad's signature before they install."}
      >
        {canInstall ? (
          <button className="btn btn-small" onClick={() => void installUpdate()}>
            {update.kind === "error" ? "Try again" : "Install and restart"}
          </button>
        ) : (
          <button className="btn btn-small" disabled={busy} onClick={() => void checkForUpdates()}>
            Check for updates
          </button>
        )}
      </Row>
      {update.kind === "available" && update.notes && <p className="settings-hint faint settings-notes">{update.notes}</p>}
    </section>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="settings-row">
      <div className="settings-text">
        <span className="settings-label">{label}</span>
        {hint && <span className="settings-hint faint">{hint}</span>}
      </div>
      {children}
    </div>
  );
}

function Toggle({ on, label, onChange }: { on: boolean; label: string; onChange: () => void }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} className={`switch ${on ? "is-on" : ""}`} onClick={onChange}>
      <span />
    </button>
  );
}
