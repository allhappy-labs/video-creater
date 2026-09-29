import * as SwitchPrimitive from "@radix-ui/react-switch";
import { forwardRef, type ComponentPropsWithoutRef, type ComponentRef } from "react";
import { cn } from "@/lib/utils";

/** A compact Radix switch: `--hover` track that fills with the accent when checked. */
export const Switch = forwardRef<ComponentRef<typeof SwitchPrimitive.Root>, ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>>(
  function Switch({ className, ...props }, ref) {
    return (
      <SwitchPrimitive.Root
        ref={ref}
        className={cn(
          "peer inline-flex h-[18px] w-8 shrink-0 cursor-default items-center rounded-full bg-hover p-0.5 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40 data-[state=checked]:bg-primary motion-reduce:transition-none",
          className,
        )}
        {...props}
      >
        <SwitchPrimitive.Thumb className="block h-3.5 w-3.5 rounded-full bg-foreground shadow transition-transform data-[state=checked]:translate-x-3.5 motion-reduce:transition-none" />
      </SwitchPrimitive.Root>
    );
  },
);
