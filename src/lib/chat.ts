import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { ChatDeltaEvent, ChatDoneEvent, ChatNote, ChatScope, ChatStep, ChatStepEvent } from "./api";
import { useEvent } from "./useEvent";

export interface ScopeChoice {
  scope: ChatScope;
  /** Shown on the chip, e.g. the note's title or the theme's name. */
  label: string;
}

/** What is streaming in for one chat right now. */
export interface Live {
  question: string;
  steps: ChatStep[];
  notes: ChatNote[];
  text: string;
}

export const emptyLive = (question = ""): Live => ({ question, steps: [], notes: [], text: "" });

/**
 * Answers in progress, by chat id. Lives at the app level so an answer keeps
 * streaming while you look at something else and is there when you return.
 */
export function useChatLive() {
  const qc = useQueryClient();
  const [live, setLive] = useState<Record<string, Live>>({});
  const patch = (id: string, f: (l: Live) => Live) => setLive((m) => ({ ...m, [id]: f(m[id] ?? emptyLive()) }));

  useEvent<ChatStepEvent>("chat-step", (e) =>
    patch(e.chatId, (l) => ({ ...l, steps: [...l.steps, e.step], notes: [...l.notes.filter((n) => !e.notes.some((x) => x.n === n.n)), ...e.notes] })),
  );
  useEvent<ChatDeltaEvent>("chat-delta", (e) => patch(e.chatId, (l) => ({ ...l, text: l.text + e.text })));
  useEvent<ChatDoneEvent>("chat-done", (e) => {
    void Promise.all([qc.invalidateQueries({ queryKey: ["chat", e.chatId] }), qc.invalidateQueries({ queryKey: ["chats"] })]).then(() =>
      setLive((m) => {
        const { [e.chatId]: _, ...rest } = m;
        return rest;
      }),
    );
  });

  /** Mark a question as sent (shows it at once, before any event arrives). */
  const begin = (id: string, question: string) => setLive((m) => ({ ...m, [id]: { ...(m[id] ?? emptyLive()), question } }));
  return { live, begin };
}
