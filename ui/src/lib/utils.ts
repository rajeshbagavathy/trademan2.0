import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function inr(ticks: number) {
  const rupees = ticks / 100;
  const formatted = Math.abs(rupees).toLocaleString("en-IN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  return rupees < 0 ? `−${formatted}` : rupees > 0 ? `+${formatted}` : formatted;
}

export function px(ticks: number | null) {
  if (ticks == null) return "—";
  return (ticks / 100).toLocaleString("en-IN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}
