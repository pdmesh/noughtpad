import type { SelectHTMLAttributes } from "react";
import { ChevronsUpDown } from "lucide-react";

/**
 * A dropdown that matches the app's buttons and fields when closed, and still opens
 * the system's own menu (native, keyboard- and screen-reader-friendly) when clicked.
 */
export function Select({ className = "", children, ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <span className={`select ${className}`}>
      <select {...props}>{children}</select>
      <ChevronsUpDown size={13} className="select-chevron" aria-hidden />
    </span>
  );
}
