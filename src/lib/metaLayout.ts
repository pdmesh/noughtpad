// Layout decisions for a note's details line (labels, source, date).

/** The note's first line, when it is a Markdown heading: it then stands in as the
 *  title and the details line sits under it. `to` is the end of that line. */
export function headingLine(doc: string): { level: number; to: number } | null {
  const end = doc.indexOf("\n");
  const first = end === -1 ? doc : doc.slice(0, end);
  const m = /^(#{1,6})[ \t]+\S/.exec(first);
  return m ? { level: m[1].length, to: first.length } : null;
}

/** How many leading items (by width) fit on one line of `available` px. When not
 *  all fit, `reserve` px are kept for the "+N" chip that stands in for the rest. */
export function fitCount(widths: number[], available: number, gap: number, reserve: number): number {
  const total = widths.reduce((sum, w, i) => sum + w + (i ? gap : 0), 0);
  if (total <= available) return widths.length;
  let used = 0;
  let n = 0;
  for (const w of widths) {
    const next = used + (n ? gap : 0) + w;
    if (next + gap + reserve > available) break;
    used = next;
    n++;
  }
  return n;
}

/** Whether an element is in the note's own text: inside the editor, but not in the
 *  details line that can sit inside it (editor/headerSlot.ts). */
export function inNoteText(el: Element | null | undefined): boolean {
  return !!el?.closest(".cm-content") && !el.closest(".note-header-slot");
}
