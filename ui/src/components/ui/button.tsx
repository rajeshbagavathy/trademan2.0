import type { ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/utils";

export function Button(
  props: ButtonHTMLAttributes<HTMLButtonElement> & { tone?: "primary" | "ghost" | "quiet" },
) {
  const { tone = "quiet", className, ...rest } = props;
  return (
    <button
      className={cn(
        "inline-flex h-9 items-center justify-center rounded-md px-3 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-40",
        tone === "primary" && "bg-indigo-600 text-white hover:bg-indigo-500",
        tone === "ghost" && "border border-slate-300 bg-white text-slate-800 hover:bg-slate-50",
        tone === "quiet" && "bg-slate-100 text-slate-800 hover:bg-slate-200",
        className,
      )}
      {...rest}
    />
  );
}
