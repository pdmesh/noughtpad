import { useEffect, useRef } from "react";
import { onEvent } from "./api";

/**
 * Subscribe to a backend event for the component's lifetime. Subscribing is
 * asynchronous, so an unmount (or StrictMode's double effect) can happen
 * before it resolves; the late subscription is then removed at once instead
 * of leaking a second handler.
 */
export function useEvent<T>(name: string, handler: (payload: T) => void) {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => {
    let off: (() => void) | undefined;
    let gone = false;
    void onEvent<T>(name, (p) => ref.current(p)).then((u) => {
      if (gone) u();
      else off = u;
    });
    return () => {
      gone = true;
      off?.();
    };
  }, [name]);
}
