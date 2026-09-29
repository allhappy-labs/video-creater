import * as SliderPrimitive from "@radix-ui/react-slider";
import { forwardRef, type ComponentPropsWithoutRef, type ComponentRef } from "react";
import { cn } from "@/lib/utils";

interface SliderProps extends ComponentPropsWithoutRef<typeof SliderPrimitive.Root> {
  /** Accessible name for the thumb. */
  readonly label: string;
  /** Human-readable value announced for the thumb, for example "150%". */
  readonly valueText?: string;
}

/** A single-thumb Radix slider: `--hover` track, accent range and a round thumb. */
export const Slider = forwardRef<ComponentRef<typeof SliderPrimitive.Root>, SliderProps>(function Slider(
  { label, valueText, className, ...props },
  ref,
) {
  return (
    <SliderPrimitive.Root
      ref={ref}
      className={cn("relative flex h-5 touch-none select-none items-center data-[disabled]:opacity-40", className)}
      {...props}
    >
      <SliderPrimitive.Track className="relative h-[3px] grow overflow-hidden rounded-full bg-hover">
        <SliderPrimitive.Range className="absolute h-full bg-muted-foreground" />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb
        aria-label={label}
        aria-valuetext={valueText}
        className="block h-3 w-3 rounded-full bg-foreground shadow focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />
    </SliderPrimitive.Root>
  );
});
