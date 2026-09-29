import * as MenuPrimitive from "@radix-ui/react-context-menu";
import { ChevronRight } from "lucide-react";
import { forwardRef, type ComponentPropsWithoutRef, type ComponentRef } from "react";
import { cn } from "@/lib/utils";

export const ContextMenu = MenuPrimitive.Root;
export const ContextMenuTrigger = MenuPrimitive.Trigger;

export const ContextMenuContent = forwardRef<
  ComponentRef<typeof MenuPrimitive.Content>,
  ComponentPropsWithoutRef<typeof MenuPrimitive.Content>
>(function ContextMenuContent({ className, ...props }, ref) {
  return (
    <MenuPrimitive.Portal>
      <MenuPrimitive.Content
        ref={ref}
        collisionPadding={8}
        className={cn(
          "z-50 max-h-[var(--radix-context-menu-content-available-height)] min-w-52 overflow-y-auto rounded-control border border-line bg-popover p-1 text-popover-foreground shadow-xl",
          className,
        )}
        {...props}
      />
    </MenuPrimitive.Portal>
  );
});

export const ContextMenuItem = forwardRef<
  ComponentRef<typeof MenuPrimitive.Item>,
  ComponentPropsWithoutRef<typeof MenuPrimitive.Item>
>(function ContextMenuItem({ className, ...props }, ref) {
  return (
    <MenuPrimitive.Item
      ref={ref}
      className={cn(
        "flex cursor-default select-none items-center gap-2 rounded-md px-2 py-1 text-[13px] outline-none data-[disabled]:text-dim data-[highlighted]:bg-raised",
        className,
      )}
      {...props}
    />
  );
});

export const ContextMenuSeparator = forwardRef<
  ComponentRef<typeof MenuPrimitive.Separator>,
  ComponentPropsWithoutRef<typeof MenuPrimitive.Separator>
>(function ContextMenuSeparator({ className, ...props }, ref) {
  return <MenuPrimitive.Separator ref={ref} className={cn("-mx-1 my-1 h-px bg-line", className)} {...props} />;
});

export const ContextMenuSub = MenuPrimitive.Sub;

export const ContextMenuSubTrigger = forwardRef<
  ComponentRef<typeof MenuPrimitive.SubTrigger>,
  ComponentPropsWithoutRef<typeof MenuPrimitive.SubTrigger>
>(function ContextMenuSubTrigger({ className, children, ...props }, ref) {
  return (
    <MenuPrimitive.SubTrigger
      ref={ref}
      className={cn(
        "flex cursor-default select-none items-center gap-2 rounded-md px-2 py-1 text-[13px] outline-none data-[disabled]:text-dim data-[highlighted]:bg-raised data-[state=open]:bg-raised",
        className,
      )}
      {...props}
    >
      {children}
      <ChevronRight className="ml-auto h-3.5 w-3.5 shrink-0 text-dim" aria-hidden />
    </MenuPrimitive.SubTrigger>
  );
});

export const ContextMenuSubContent = forwardRef<
  ComponentRef<typeof MenuPrimitive.SubContent>,
  ComponentPropsWithoutRef<typeof MenuPrimitive.SubContent>
>(function ContextMenuSubContent({ className, ...props }, ref) {
  return (
    <MenuPrimitive.Portal>
      <MenuPrimitive.SubContent
        ref={ref}
        collisionPadding={8}
        className={cn("z-50 min-w-40 rounded-control border border-line bg-popover p-1 text-popover-foreground shadow-xl", className)}
        {...props}
      />
    </MenuPrimitive.Portal>
  );
});
