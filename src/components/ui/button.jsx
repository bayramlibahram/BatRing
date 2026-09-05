import { cn } from "../../lib/utils";

const variants = {
  default:
    "border-slate-900 bg-slate-900 text-white hover:border-slate-800 hover:bg-slate-800",
  outline:
    "border-slate-300 bg-white text-slate-700 hover:border-slate-400 hover:bg-slate-50",
  destructive:
    "border-red-600 bg-red-600 text-white hover:border-red-700 hover:bg-red-700",
  ghost:
    "border-transparent bg-transparent text-slate-600 hover:bg-slate-100 hover:text-slate-900",
};

export function Button({ className, variant = "default", type = "button", ...props }) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex h-8 items-center justify-center rounded-md border px-3 text-xs font-medium shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-400 focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50",
        variants[variant],
        className,
      )}
      {...props}
    />
  );
}

