import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Tooltip } from "./tooltip";

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  readonly label: string;
  readonly tooltip?: ReactNode;
  readonly size?: "sm" | "md";
  readonly active?: boolean;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { label, tooltip, size = "md", active = false, className, children, ...props },
  ref,
) {
  const button = (
    <button
      ref={ref}
      type="button"
      aria-label={label}
      className={cn(
        "grid place-items-center rounded-control text-muted-foreground transition-colors hover:bg-raised hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40",
        size === "md" ? "h-8 w-8" : "h-[26px] w-[26px] rounded-md",
        active && "bg-raised text-foreground",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
  return <Tooltip content={tooltip ?? label}>{button}</Tooltip>;
});
