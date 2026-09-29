import { Toast, ToastAction, ToastClose, ToastDescription, ToastProvider, ToastTitle, ToastViewport } from "./toast";

/** How long a toast stays; Radix pauses the timer while the toast is hovered or focused. */
const toastDurationMs = 6_000;

interface ToasterItem {
  readonly id: string;
  readonly title: string;
  readonly description?: string;
  readonly action?: { readonly label: string; onSelect(): void };
}

/** Renders a toast queue (the editor's `ui.toasts`); closing or timing out a toast calls `onDismiss`. */
export function Toaster({ toasts, onDismiss }: { readonly toasts: readonly ToasterItem[]; onDismiss(id: string): void }) {
  return (
    <ToastProvider duration={toastDurationMs} label="Notification">
      {toasts.map((toast) => (
        <Toast key={toast.id} onOpenChange={(open) => !open && onDismiss(toast.id)}>
          <div className="min-w-0 flex-1">
            <ToastTitle className="truncate font-medium">{toast.title}</ToastTitle>
            {toast.description && <ToastDescription>{toast.description}</ToastDescription>}
          </div>
          {toast.action && (
            <ToastAction altText={toast.action.label} onClick={toast.action.onSelect}>
              {toast.action.label}
            </ToastAction>
          )}
          <ToastClose />
        </Toast>
      ))}
      <ToastViewport />
    </ToastProvider>
  );
}
