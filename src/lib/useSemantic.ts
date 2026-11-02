import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, onEvent, type SemanticStatus } from "./api";

/** Live status of local semantic search (model download, indexing, ready…). */
export function useSemanticStatus(): SemanticStatus {
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ["semantic-status"], queryFn: api.semanticStatus, staleTime: Infinity });
  useEffect(() => {
    let off = () => {};
    void onEvent<SemanticStatus>("semantic-status", (s) => {
      qc.setQueryData(["semantic-status"], s);
      // New vectors change related notes and search results.
      if (s.state === "ready" || s.state === "indexing") void qc.invalidateQueries({ queryKey: ["related"] });
    }).then((u) => (off = u));
    return () => off();
  }, [qc]);
  return data ?? { state: "off" };
}

export function describeStatus(s: SemanticStatus): string | null {
  switch (s.state) {
    case "downloading":
      return s.total ? `Downloading model… ${Math.round((100 * s.received) / s.total)}%` : "Downloading model…";
    case "loading":
      return "Loading model…";
    case "indexing":
      return `Connecting notes… ${s.done.toLocaleString()} of ${s.total.toLocaleString()}`;
    case "error":
      return "Semantic search paused";
    default:
      return null;
  }
}
