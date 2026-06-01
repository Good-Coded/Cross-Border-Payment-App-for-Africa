import React, { useEffect } from 'react';
import { useLocation } from 'react-router-dom';

class ErrorBoundary extends React.Component {
  constructor(props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error) {
    return { hasError: true };
  }

  componentDidCatch(error, errorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
    // Optionally send to monitoring service
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen flex items-center justify-center bg-gray-950 text-white">
          <div className="text-center">
            <h1 className="text-2xl font-bold mb-4">Something went wrong.</h1>
            <p>Please refresh the page.</p>
          </div>
        </div>
      );
    }

    return <ErrorBoundaryWrapper>{this.props.children}</ErrorBoundaryWrapper>;
  }
}

// Wrapper component to handle route changes and reset error boundary
function ErrorBoundaryWrapper({ children }) {
  const location = useLocation();

  useEffect(() => {
    // Reset error boundary when route changes
    // This is handled by the key prop on the ErrorBoundary in App.jsx
  }, [location]);

  return children;
}

export default ErrorBoundary;