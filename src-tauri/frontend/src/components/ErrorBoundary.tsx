import { Component, type ErrorInfo, type ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

/** 捕获子树渲染错误，避免单个组件崩溃导致整个窗口白屏。 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("ErrorBoundary 捕获到错误:", error, info);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-full w-full items-center justify-center p-4">
          <div className="max-w-md text-center text-sm text-destructive">
            <p className="font-medium">界面渲染出错</p>
            <p className="mt-1 break-all font-mono text-xs">{this.state.error.message}</p>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
