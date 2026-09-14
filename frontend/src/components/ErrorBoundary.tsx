import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Catches a render-time crash in any page so one bad page doesn't blank the
 * entire dashboard (React error boundaries must be class components — there
 * is no hook equivalent). Data-fetch errors already have their own inline
 * `ErrorState` handling (see `usePolled`); this is the backstop for
 * everything else (a bad render, an unexpected shape in API data, etc).
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Unhandled error in page render:", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="empty-state">
          <div className="empty-state-title">Something went wrong</div>
          <div>{this.state.error.message}</div>
          <button
            type="button"
            className="export-btn"
            style={{ marginTop: 14 }}
            onClick={() => {
              this.setState({ error: null });
              window.location.assign("/");
            }}
          >
            Back to Overview
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
