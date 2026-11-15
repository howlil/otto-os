import { useId } from "react";
import type { InputHTMLAttributes, Ref } from "react";

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  ref?: Ref<HTMLInputElement>;
  label: string;
  hideLabel?: boolean;
  error?: string;
}

export function Input({
  ref,
  label,
  hideLabel = false,
  error,
  id,
  className = "",
  ...props
}: InputProps) {
  const generatedId = useId();
  const controlId = id ?? generatedId;
  const errorId = error ? `${controlId}-error` : undefined;
  const describedBy = [props["aria-describedby"], errorId].filter(Boolean).join(" ") || undefined;

  return (
    <div className="grid min-w-0 gap-1.5 text-left">
      <label
        htmlFor={controlId}
        className={hideLabel ? "sr-only" : "text-sm font-medium text-ink-soft"}
      >
        {label}
      </label>
      <input
        {...props}
        ref={ref}
        id={controlId}
        aria-invalid={error ? true : props["aria-invalid"]}
        aria-describedby={describedBy}
        className={[
          "h-11 sm:h-12 w-full rounded-lg border bg-surface px-4 text-base text-ink placeholder:text-muted transition-[border-color,background-color] duration-150 ease-[cubic-bezier(0.22,1,0.36,1)] hover:border-ink/40 focus-visible:border-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink disabled:cursor-not-allowed disabled:opacity-50",
          error ? "border-danger" : "border-line-strong",
          className,
        ]
          .filter(Boolean)
          .join(" ")}
      />
      {error ? (
        <p id={errorId} role="alert" className="text-xs leading-relaxed text-danger">
          {error}
        </p>
      ) : null}
    </div>
  );
}
