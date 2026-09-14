import { Component, type ReactNode } from 'react';
import i18n from '@/i18n';

interface Props {
  children: ReactNode;
}
interface State {
  hasError: boolean;
  error: string;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, error: '' };

  static getDerivedStateFromError(e: Error): State {
    console.error('[ErrorBoundary]', e);
    return { hasError: true, error: e.message };
  }

  componentDidCatch(_e: Error, info: React.ErrorInfo) {
    console.error('[ErrorBoundary] componentStack:', info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          className="flex items-center justify-center h-screen"
          style={{ backgroundColor: 'var(--app-bg)', color: 'var(--app-fg)' }}
        >
          <div className="text-center space-y-4">
            <h2 className="text-xl font-bold">{i18n.t('app.errorTitle')}</h2>
            <p className="text-sm opacity-70">{this.state.error}</p>
            <button
              className="px-4 py-2 rounded border"
              onClick={() => this.setState({ hasError: false })}
              style={{ borderColor: 'var(--app-border)' }}
            >
              {i18n.t('button.retry')}
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
