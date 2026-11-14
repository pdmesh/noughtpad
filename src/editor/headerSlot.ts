// A place inside the editor, right under a first-line heading, where the note's
// details line (labels, source, date) can sit. The heading then reads as the
// title with its details beneath it, as a title field would. The slot is one
// host element owned by NotePane (React renders into it with a portal); the
// editor only positions it, and leaves events and DOM changes inside it alone.
import { type EditorState, type Extension, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, WidgetType } from "@codemirror/view";
import { headingLine } from "../lib/metaLayout";

class SlotWidget extends WidgetType {
  constructor(readonly host: HTMLElement) {
    super();
  }
  eq(other: SlotWidget) {
    return other.host === this.host;
  }
  toDOM() {
    return this.host;
  }
  /** Clicks and keys inside the details line are its own, not the editor's. */
  ignoreEvent() {
    return true;
  }
  get estimatedHeight() {
    return 42;
  }
}

const hasHeading = (s: EditorState) => headingLine(s.doc.line(1).text) !== null;

/** A field inside the slot (not the editable editor around it) that should take the click. */
const keepsFocus = (host: HTMLElement, t: EventTarget | null) => {
  const field = t instanceof Element ? t.closest("input, textarea, select, [contenteditable=true]") : null;
  return !!field && host.contains(field);
};

export function headerSlot(host: HTMLElement, onHeading: (has: boolean) => void): Extension {
  // The editor ignores clicks here, but the browser would still drop a caret into
  // the editable text next to the slot (moving the note's cursor) on a click on
  // plain text or a button. Fields keep their normal clicks.
  if (!host.dataset.slotReady) {
    host.dataset.slotReady = "1";
    host.addEventListener("mousedown", (e) => {
      if (!keepsFocus(host, e.target)) e.preventDefault();
    });
  }
  const widget = new SlotWidget(host);
  const build = (s: EditorState): DecorationSet => {
    if (!hasHeading(s)) return Decoration.none;
    return Decoration.set([Decoration.widget({ widget, block: true, side: 1 }).range(s.doc.line(1).to)]);
  };
  const field = StateField.define<DecorationSet>({
    create: build,
    update: (value, tr) => (tr.docChanged ? build(tr.state) : value),
    provide: (f) => EditorView.decorations.from(f),
  });
  const report = ViewPlugin.define((view) => {
    let last = hasHeading(view.state);
    queueMicrotask(() => onHeading(last));
    return {
      update(u) {
        if (!u.docChanged) return;
        const now = hasHeading(u.state);
        if (now !== last) onHeading((last = now));
      },
    };
  });
  return [field, report];
}
