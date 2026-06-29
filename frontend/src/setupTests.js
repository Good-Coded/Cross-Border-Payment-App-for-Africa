
// Mock Sentry
jest.mock('@sentry/react', () => ({
	withProfiler: (component) => component,
	captureException: jest.fn(),
	captureMessage: jest.fn(),
}));
import '@testing-library/jest-dom';
