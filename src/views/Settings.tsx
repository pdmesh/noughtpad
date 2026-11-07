import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronRight, FolderOpen } from "lucide-react";
import { api, isApiError, type VaultInfo } from "../lib/api";
import { describeStatus, useSemanticStatus } from "../lib/useSemantic";
import { checkForUpdates, describeUpdate, installUpdate, useUpdateState } from "../lib/updates";
import { describeSync, syncNotices, useSync } from "../lib/sync";
import { useState } from "react";
import { useLlm } from "./Models";
import { FONTS, SIZES, loadStyle, saveStyle, type FontChoice, type TextStyle } from "../lib/textStyle";
import { thisDevice } from "../lib/platform";

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
        {vault.canSwitchVault ? (
          <Row label={vault.path} hint="Plain Markdown files. Noughtpad keeps its index in a .noughtpad folder inside it.">
            <button className="btn btn-small" onClick={onSwitchVault}>
              <FolderOpen size={13} aria-hidden /> Switch…
            </button>
          </Row>
        ) : (
          // Mobile: one fixed folder in the app's own storage (docs/MOBILE.md § Where notes live),
          // so its sandbox path means nothing to the user and there's nothing to switch to.
          <Row label="Noughtpad" hint="Plain Markdown files in this app's Noughtpad folder, which your phone's Files app can open." />
        )}
        <Row label="Rebuild the search index" hint="Only needed if search seems out of date. Your notes are not touched.">
          <button className="btn btn-small" onClick={onRebuildIndex}>
            Rebuild
          </button>
        </Row>
      </section>

      <Sync />

      <section className="settings-section">
        <h3 className="ideas-section-title">Finding connections</h3>
        <Row
          label="Search by meaning, related notes and ideas over time"
          hint={`Uses a small model that runs on ${thisDevice} (about 65 MB, downloaded once). ${semanticLine}.`}
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

      <WritingSettings />

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

/** Noughtpad Sync, in builds that include it. */
function Sync() {
  const qc = useQueryClient();
  const sync = useSync();
  const [signingIn, setSigningIn] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toggling, setToggling] = useState(false);
  if (!sync.available) return null;
  const refresh = () => qc.invalidateQueries({ queryKey: ["sync"] });
  const fail = (e: unknown) => setError(isApiError(e) ? e.message : String(e));
  const signIn = () => {
    setSigningIn(true);
    setError(null);
    api
      .syncSignIn()
      .then(refresh, fail)
      .finally(() => setSigningIn(false));
  };
  const folder = `Noughtpad/${sync.notebookName ?? "Notes"}`;
  const notices = sync.enabled ? syncNotices(sync.last) : [];
  return (
    <section className="settings-section">
      <h3 className="ideas-section-title">Sync</h3>
      {!sync.configured ? (
        <Row label="Google Drive" hint="This build can't sign in to Google: it was built without Noughtpad's Google sign-in client." >
          <span />
        </Row>
      ) : !sync.signedIn ? (
        <Row
          label="Google Drive"
          hint="Keep this notebook the same on your other devices through your own Google Drive. Noughtpad can only see the files it creates there."
        >
          <button className="btn btn-small" disabled={signingIn} onClick={signIn}>
            {signingIn ? "Waiting for the browser…" : "Sign in with Google…"}
          </button>
        </Row>
      ) : (
        <>
          <Row label={`Google Drive${sync.account ? ` · ${sync.account}` : ""}`} hint={`Signed in. The sign-in is kept in ${thisDevice}'s keychain.`}>
            <button className="btn btn-small" onClick={() => void api.syncSignOut().then(refresh, fail)}>
              Sign out
            </button>
          </Row>
          <Row
            label="Sync this notebook"
            hint={
              sync.enabled
                ? `${describeSync(sync)}. In Drive: ${folder}.`
                : `Copies your notes to ${folder} in Drive and keeps every device that syncs it up to date. The search index stays on each computer.`
            }
          >
            <Toggle
              on={sync.enabled}
              label="Sync this notebook"
              disabled={toggling}
              onChange={() => {
                setToggling(true);
                void api
                  .syncSetEnabled(!sync.enabled)
                  .then(refresh, fail)
                  .finally(() => setToggling(false));
              }}
            />
          </Row>
          {sync.enabled && (
            <Row label="Sync now" hint="Noughtpad syncs on its own every minute and a few seconds after you stop typing.">
              <button className="btn btn-small" disabled={sync.state === "syncing"} onClick={() => void api.syncNow()}>
                Sync now
              </button>
            </Row>
          )}
        </>
      )}
      {notices.map((n) => (
        <p key={n} className="settings-hint faint settings-notes">
          {n}
        </p>
      ))}
      {error && <p className="settings-hint settings-notes settings-error">{error}</p>}
    </section>
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

/** The note text's font and size (this computer only). */
function WritingSettings() {
  const [style, setStyle] = useState<TextStyle>(loadStyle);
  const change = (patch: Partial<TextStyle>) => {
    const next = { ...style, ...patch };
    setStyle(next);
    saveStyle(next);
  };
  return (
    <section className="settings-section">
      <h3 className="ideas-section-title">Writing</h3>
      <Row label="Font" hint="For note text. System uses your computer's own font.">
        <span className="settings-controls">
          {style.font === "custom" && (
            <input
              className="settings-input"
              aria-label="Font name"
              placeholder="Font name, e.g. Charter"
              spellCheck={false}
              value={style.custom}
              onChange={(e) => change({ custom: e.target.value })}
            />
          )}
          <select className="settings-select" aria-label="Font" value={style.font} onChange={(e) => change({ font: e.target.value as FontChoice })}>
            {FONTS.map((f) => (
              <option key={f.value} value={f.value}>
                {f.label}
              </option>
            ))}
          </select>
        </span>
      </Row>
      <Row label="Text size" hint="System uses your computer's default size.">
        <select
          className="settings-select"
          aria-label="Text size"
          value={style.size ?? ""}
          onChange={(e) => change({ size: e.target.value ? Number(e.target.value) : null })}
        >
          <option value="">System</option>
          {SIZES.map((n) => (
            <option key={n} value={n}>
              {n} px
            </option>
          ))}
        </select>
      </Row>
      <p className="settings-sample" aria-hidden>
        The quick brown fox jumps over the lazy dog.
      </p>
    </section>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children?: React.ReactNode }) {
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

function Toggle({ on, label, onChange, disabled }: { on: boolean; label: string; onChange: () => void; disabled?: boolean }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} className={`switch ${on ? "is-on" : ""}`} onClick={onChange} disabled={disabled}>
      <span />
    </button>
  );
}
