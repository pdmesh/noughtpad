import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "katex/dist/katex.min.css";
import "./styles.css";
import { App } from "./App";
import { applyStyle, loadStyle } from "./lib/textStyle";

// A file dropped where nothing handles it would make the webview open it in
// place of the app. Swallow those (the editor and note page handle their own
// drops first, as these listeners run last). Text drags are left alone.
const isFileDrag = (e: DragEvent) => Array.from(e.dataTransfer?.types ?? []).includes("Files");
window.addEventListener("dragover", (e) => {
  if (isFileDrag(e)) e.preventDefault();
});
window.addEventListener("drop", (e) => {
  if (isFileDrag(e)) e.preventDefault();
});

// Before the first render, so the note text never jumps to the chosen font.
applyStyle(loadStyle());

const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000, refetchOnWindowFocus: false, retry: 1 } },
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </StrictMode>,
);
