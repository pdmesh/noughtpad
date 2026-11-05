// Small shared pieces for rich blocks: the hover tool buttons (zoom, copy,
// edit as Markdown) and a normal-height cursor beside tall widgets.

const svg = (body: string) =>
  `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;

export const ICONS = {
  code: svg('<path d="m16 18 6-6-6-6"/><path d="m8 6-6 6 6 6"/>'),
  copy: svg('<rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>'),
  check: svg('<path d="M20 6 9 17l-5-5"/>'),
  zoom: svg('<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5M11 8v6M8 11h6"/>'),
};

export function toolButton(title: string, icon: string, onClick: (button: HTMLButtonElement) => void): HTMLButtonElement {
  const b = document.createElement("button");
  b.className = "cm-widget-tool";
  b.title = title;
  b.setAttribute("aria-label", title);
  b.tabIndex = -1;
  b.innerHTML = icon;
  // Keep the editor's focus and cursor where they are.
  b.addEventListener("mousedown", (e) => {
    e.preventDefault();
    e.stopPropagation();
  });
  b.addEventListener("click", (e) => {
    e.preventDefault();
    e.stopPropagation();
    onClick(b);
  });
  return b;
}

/** A row of hover tools. `boxed` gives them a backdrop, for use over pictures. */
export function toolBar(buttons: HTMLElement[], boxed = false): HTMLElement {
  const bar = document.createElement("span");
  bar.className = `cm-widget-tools${boxed ? " is-boxed" : ""}`;
  bar.append(...buttons);
  return bar;
}

/** A copy button that ticks for a moment once copied. */
export function copyButton(text: () => string, title = "Copy"): HTMLButtonElement {
  return toolButton(title, ICONS.copy, (b) => {
    void navigator.clipboard
      ?.writeText(text())
      .then(() => {
        b.innerHTML = ICONS.check;
        b.classList.add("is-done");
        setTimeout(() => {
          b.innerHTML = ICONS.copy;
          b.classList.remove("is-done");
        }, 1200);
      })
      .catch(() => {});
  });
}

/**
 * Where to draw the cursor beside a tall widget: one line high at the widget's
 * bottom edge, instead of the widget's full height. `pos` 0 is before it.
 */
export function caretRect(dom: HTMLElement, pos: number) {
  const r = dom.getBoundingClientRect();
  if (!r.height) return null;
  const size = parseFloat(getComputedStyle(dom).fontSize) || 16;
  const h = Math.min(r.height, size * 1.45);
  const x = pos === 0 ? r.left : r.right;
  return { left: x, right: x, top: r.bottom - h, bottom: r.bottom };
}
