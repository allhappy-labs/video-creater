import { Component, type ErrorInfo, type ReactNode } from "react";

interface AppErrorBoundaryProps {
  children: ReactNode;
  onRecover: () => void;
}

interface AppErrorBoundaryState {
  error: Error | null;
}

export class AppErrorBoundary extends Component<
  AppErrorBoundaryProps,
  AppErrorBoundaryState
> {
  override state: AppErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): AppErrorBoundaryState {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Application render failed", error, info);
  }

  private recover = () => {
    this.props.onRecover();
    this.setState({ error: null });
  };

  override render() {
    if (!this.state.error) return this.props.children;

    return (
      <main
        aria-label="Editor recovery"
        className="grid min-h-screen place-items-center bg-[#121314] p-6 text-foreground"
      >
        <div
          role="alert"
          className="w-full max-w-md rounded-md border border-destructive/30 bg-[#1d2021] p-4"
        >
          <h1 className="text-sm font-semibold">The editor stopped rendering</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            Reload the app, then reopen your saved project.
          </p>
          <button
            type="button"
            className="mt-4 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            onClick={this.recover}
          >
            Reload app
          </button>
        </div>
      </main>
    );
  }
}
