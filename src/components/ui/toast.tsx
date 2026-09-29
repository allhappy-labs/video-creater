import * as ToastPrimitive from "@radix-ui/react-toast";
import { X } from "lucide-react";
import { forwardRef, type ComponentPropsWithoutRef, type ComponentRef } from "react";
import { cn } from "@/lib/utils";

export const ToastProvider = ToastPrimitive.Provider;
export const ToastTitle = ToastPrimitive.Title;

export const ToastViewport = forwardRef<
  ComponentRef<typeof ToastPrimitive.Viewport>,
  ComponentPropsWithoutRef<typeof ToastPrimitive.Viewport>
>(function ToastViewport({ className, ...props }, ref) {
  return (
    <ToastPrimitive.Viewport
      ref={ref}
      className={cn("fixed bottom-4 right-4 z-[60] flex w-[min(360px,calc(100vw-32px))] flex-col gap-2 outline-none", className)}
      {...props}
    />
  );
});

export const Toast = forwardRef<ComponentRef<typeof ToastPrimitive.Root>, ComponentPropsWithoutRef<typeof ToastPrimitive.Root>>(
  function Toast({ className, ...props }, ref) {
    return (
      <ToastPrimitive.Root
        ref={ref}
        className={cn(
          "flex items-center gap-2 rounded-control border border-line bg-popover px-3 py-2.5 text-[13px] text-popover-foreground shadow-xl",
          className,
        )}
        {...props}
      />
    );
  },
);

export const ToastDescription = forwardRef<
  ComponentRef<typeof ToastPrimitive.Description>,
  ComponentPropsWithoutRef<typeof ToastPrimitive.Description>
>(function ToastDescription({ className, ...props }, ref) {
  return <ToastPrimitive.Description ref={ref} className={cn("text-[12px] text-muted-foreground", className)} {...props} />;
});

export const ToastAction = forwardRef<ComponentRef<typeof ToastPrimitive.Action>, ComponentPropsWithoutRef<typeof ToastPrimitive.Action>>(
  function ToastAction({ className, ...props }, ref) {
    return (
      <ToastPrimitive.Action
        ref={ref}
        className={cn(
          "h-7 shrink-0 rounded-control bg-raised px-2.5 text-[12px] font-medium text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
          className,
        )}
        {...props}
      />
    );
  },
);

export function ToastClose({ label = "Dismiss" }: { readonly label?: string }) {
  return (
    <ToastPrimitive.Close
      aria-label={label}
      className="grid h-6 w-6 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-raised hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <X className="h-3.5 w-3.5" aria-hidden />
    </ToastPrimitive.Close>
  );
}
