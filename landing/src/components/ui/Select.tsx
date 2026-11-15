import { useId } from "react";
import type { SelectHTMLAttributes } from "react";
import { ChevronDown } from "lucide-react";

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label: string;
  placeholder: string;
  options: readonly SelectOption[];
}

export function Select({ label, placeholder, options, id, className = "", ...props }: SelectProps) {
  const generatedId = useId();
  const controlId = id ?? generatedId;

  return (
    <div className="grid gap-1.5 text-left">
      <label htmlFor={controlId} className="text-sm font-medium text-ink-soft">
        {label}
      </label>
      <div className="relative">
        <select
          {...props}
          id={controlId}
          className={[
            "h-11 w-full appearance-none rounded-lg border border-line-strong bg-surface pr-10 pl-3.5 text-sm text-ink transition-[border-color] duration-150 ease-[cubic-bezier(0.22,1,0.36,1)] hover:border-ink/40 focus-visible:border-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ink disabled:cursor-not-allowed disabled:opacity-50",
            className,
          ]
            .filter(Boolean)
            .join(" ")}
        >
          <option value="">{placeholder}</option>
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        <ChevronDown
          size={16}
          strokeWidth={1.5}
          aria-hidden="true"
          className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-muted"
        />
      </div>
    </div>
  );
}
