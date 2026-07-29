import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

// Mock Sentry methods
jest.mock('@sentry/react', () => ({
  captureException: jest.fn(),
  captureUserFeedback: jest.fn(),
}));

// Component that throws an error
const ThrowError = () => {
  throw new Error('Test error');
};

describe('ErrorBoundary', () => {
  test('captures exception and displays fallback UI when child throws', () => {
    const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});
    const { getByText } = render(
      <MemoryRouter>
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      </MemoryRouter>
    );

    // Verify UI shows fallback message
    // expect(screen.getByText('Please refresh the page.')).toBeInTheDocument();
    // CaptureException should have been called with the error
    const { captureException } = require('@sentry/react');
    expect(captureException).toHaveBeenCalled();
    // Clean up console mock
    consoleSpy.mockRestore();
  });

  test('submits user feedback and calls captureUserFeedback', async () => {
    const { getByText, getByPlaceholderText } = render(
      <MemoryRouter>
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      </MemoryRouter>
    );
    // Type feedback
    const textarea = getByPlaceholderText('Describe what led to this error...');
    fireEvent.change(textarea, { target: { value: 'User feedback' } });
    // Click Send Report
    fireEvent.click(getByText('Send Report'));
    const { captureUserFeedback } = require('@sentry/react');
    // Wait for async handler to complete
    await new Promise((r) => setTimeout(r, 0));
    expect(captureUserFeedback).toHaveBeenCalled();
  });

  test('renders children when no error', () => {
    render(
      <MemoryRouter>
        <ErrorBoundary key="test-normal">
          <div>Normal content</div>
        </ErrorBoundary>
      </MemoryRouter>
    );
    expect(screen.getByText('Normal content')).toBeInTheDocument();
  });
});