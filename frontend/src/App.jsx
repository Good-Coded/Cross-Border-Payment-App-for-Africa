import React, { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route, Navigate, useLocation } from "react-router-dom";
import { Toaster } from "react-hot-toast";
import { AuthProvider, useAuth } from "./context/AuthContext";
import { ThemeProvider } from "./context/ThemeContext";

import Welcome from "./pages/Welcome";
import Login from "./pages/Login";
import Register from "./pages/Register";
import ForgotPassword from "./pages/ForgotPassword";
import ResetPassword from "./pages/ResetPassword";
import Dashboard from "./pages/Dashboard";
import SendMoney from "./pages/SendMoney";
import ReceiveMoney from "./pages/ReceiveMoney";
import SaveMoney from "./pages/SaveMoney";
import ReceiveMoney from "./pages/ReceiveMoney";
import RequestMoney from "./pages/RequestMoney";
import ScheduledPayments from "./pages/ScheduledPayments";
import TransactionHistory from "./pages/TransactionHistory";
import Profile from "./pages/Profile";
import Analytics from "./pages/Analytics";
import KYCVerification from "./pages/KYCVerification";
import BusinessSettings from "./pages/BusinessSettings";
import Swap from "./pages/Swap";
import BatchPayment from "./pages/BatchPayment";
import Webhooks from "./pages/Webhooks";
import Referrals from "./pages/Referrals";
import Sessions from "./pages/Sessions";
import Layout from "./components/Layout";
import ErrorBoundary from "./components/ErrorBoundary";

function PrivateRoute({ children }) {
  const { user, loading } = useAuth();
  if (loading)
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-950 transition-colors duration-200" role="status" aria-label="Loading">
        <div className="w-8 h-8 border-2 border-primary-500 border-t-transparent rounded-full animate-spin" />
      </div>
    );
  return user ? children : <Navigate to="/login" replace />;
}

function PublicRoute({ children }) {
  const { user, loading } = useAuth();
  if (loading) return null;
  return user ? <Navigate to="/dashboard" replace /> : children;
}

function AppRoutes() {
  const location = useLocation();

  return (
    <ErrorBoundary key={location.pathname}>
      <Routes>
        <Route
          path="/"
          element={
            <PublicRoute>
              <Welcome />
            </PublicRoute>
          }
        />
        <Route
          path="/login"
          element={
            <PublicRoute>
              <Login />
            </PublicRoute>
          }
        />
        <Route
          path="/register"
          element={
            <PublicRoute>
              <Register />
            </PublicRoute>
          }
        />
        <Route
          path="/forgot-password"
          element={
            <PublicRoute>
              <ForgotPassword />
            </PublicRoute>
          }
        />
        <Route
          path="/reset-password"
          element={
            <PublicRoute>
              <ResetPassword />
            </PublicRoute>
          }
        />
        <Route
          path="/"
          element={
            <PrivateRoute>
              <Layout />
            </PrivateRoute>
          }
        >
          <Route path="dashboard" element={<Dashboard />} />
          <Route path="send" element={<SendMoney />} />
          <Route path="batch-payments" element={<BatchPayment />} />
          <Route path="receive" element={<ReceiveMoney />} />
          <Route path="save" element={<SaveMoney />} />
          <Route path="request" element={<RequestMoney />} />
          <Route path="scheduled" element={<ScheduledPayments />} />
          <Route path="history" element={<TransactionHistory />} />
          <Route path="analytics" element={<Analytics />} />
          <Route path="profile" element={<Profile />} />
          <Route path="sessions" element={<Sessions />} />
          <Route path="kyc" element={<KYCVerification />} />
          <Route path="webhooks" element={<Webhooks />} />
          <Route path="business" element={<BusinessSettings />} />
          <Route path="swap" element={<Swap />} />
          <Route path="referrals" element={<Referrals />} />
        </Route>
      </Routes>
    </ErrorBoundary>
  );
}

export default function App() {
  const [isOffline, setIsOffline] = useState(
    typeof navigator !== "undefined" ? !navigator.onLine : false
  );

  useEffect(() => {
    const handleOffline = () => setIsOffline(true);
    const handleOnline = () => setIsOffline(false);

    window.addEventListener("offline", handleOffline);
    window.addEventListener("online", handleOnline);

    return () => {
      window.removeEventListener("offline", handleOffline);
      window.removeEventListener("online", handleOnline);
    };
  }, []);

  return (
    <>
      {isOffline && (
        <div
          role="alert"
          aria-live="assertive"
          style={{
            position: "fixed",
            top: 0,
            left: 0,
            right: 0,
            zIndex: 9999,
            backgroundColor: "#b91c1c",
            color: "#fff",
            textAlign: "center",
            padding: "10px 16px",
            fontSize: "14px",
            fontWeight: "500",
          }}
        >
          You're offline. Some features may be unavailable.
        </div>
      )}
      <AuthProvider>
        <ThemeProvider>
          <BrowserRouter>
            <Toaster
              position="top-center"
              toastOptions={{
                style: { background: "#1e293b", color: "#fff", border: "1px solid #334155" },
              }}
              containerProps={{
                "aria-live": "polite",
                "aria-atomic": "true",
              }}
            />
            <AppRoutes />
