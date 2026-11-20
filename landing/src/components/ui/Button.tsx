import type { ButtonHTMLAttributes } from "react";

export type ButtonVariant = "primary" | "secondary" | "ghost";
export type ButtonSize = "sm" | "md" | "lg";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
}

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    "bg-accent text-white border-transparent shadow-[inset_0_1px_0_rgb(255_255_255/0.35)] hover:bg-accent-hover",
  secondary:
    "bg-surface text-ink border-line-strong shadow-xs hover:bg-surface-subtle",
  ghost:
    "bg-transparent text-ink-soft border-transparent hover:bg-surface-subtle hover:text-ink",
};

const SIZE_CLASSES: Record<ButtonSize, string> = {
  sm: "h-9 px-4 text-sm",
  md: "h-11 px-5 text-sm",
  lg: "h-11 sm:h-12 px-5 text-sm",
};

export function Button({
  variant = "primary",
  size = "md",
  loading = false,
  disabled,
  className = "",
  onClick,
  children,
  ...props
}: ButtonProps) {
  const isDisabled = Boolean(disabled);
  return (
    <button
      {...props}
      disabled={isDisabled}
      aria-busy={loading || undefined}
      aria-disabled={loading || props["aria-disabled"] || undefined}
      onClick={loading ? (event) => event.preventDefault() : onClick}
      className={[
        "relative inline-flex cursor-pointer items-center justify-center rounded-lg border font-medium whitespace-nowrap transition-[background-color,border-color,color,transform] duration-150 ease-[cubic-bezier(0.22,1,0.36,1)] active:scale-[0.98] motion-reduce:transform-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink disabled:cursor-not-allowed disabled:opacity-50",
        VARIANT_CLASSES[variant],
        SIZE_CLASSES[size],
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {loading ? (
        <span
          aria-hidden="true"
          className="absolute inset-0 flex items-center justify-center"
        >
          <span className="size-4 animate-spin rounded-full border-2 border-current border-r-transparent motion-reduce:animate-none" />
        </span>
      ) : null}
      <span className={loading ? "invisible inline-flex items-center gap-2" : "inline-flex items-center gap-2"}>
        {children}
      </span>
    </button>
  );
}
