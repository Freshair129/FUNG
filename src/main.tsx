import React, { lazy, Suspense } from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import {
  isMobileTauriPlatform,
  requiresSupabaseConfig,
  resolveBodySurface,
  resolveRootRoute,
  supabaseConfigured,
} from "./lib/bootstrap";
import "./styles.css";
import "./web/LoadingScreen.css";

const LandingPage = lazy(() =>
  import("./landing/LandingPage").then((module) => ({ default: module.LandingPage })),
);
const MobileApp = lazy(() =>
  import("./mobile/MobileApp").then((module) => ({ default: module.MobileApp })),
);
const AuthCallback = lazy(() =>
  import("./web/AuthCallback").then((module) => ({ default: module.AuthCallback })),
);
const AuthGuard = lazy(() =>
  import("./web/AuthGuard").then((module) => ({ default: module.AuthGuard })),
);
const Dashboard = lazy(() =>
  import("./web/Dashboard").then((module) => ({ default: module.Dashboard })),
);

const params = new URLSearchParams(window.location.search);
const path = window.location.pathname;
const isTauriRuntime = "__TAURI_INTERNALS__" in window;
const surface = params.get("surface");
const mobileViewport = window.matchMedia(
  "(pointer: coarse) and (max-width: 760px), (pointer: coarse) and (orientation: landscape) and (max-height: 760px)",
).matches;
const isMobilePlatform = isMobileTauriPlatform(window.navigator.userAgent);
const rootRoute = resolveRootRoute({
  path,
  surface,
  isTauriRuntime,
  isMobilePlatform,
  mobileViewport,
});

function BootstrapState({ message, alert = false }: { message: string; alert?: boolean }) {
  return (
    <div className="loading-screen" role={alert ? "alert" : "status"}>
      <p className="loading-message">{message}</p>
    </div>
  );
}

function RootRouter() {
  if (rootRoute === "desktop") return <App />;

  if (!supabaseConfigured && requiresSupabaseConfig(rootRoute, isTauriRuntime)) {
    return (
      <BootstrapState
        alert
        message="ยังไม่ได้ตั้งค่า Supabase สำหรับ surface นี้ กรุณาตั้งค่า VITE_SUPABASE_URL และ VITE_SUPABASE_ANON_KEY"
      />
    );
  }

  if (rootRoute === "mobile") return <MobileApp />;
  if (rootRoute === "auth-callback") return <AuthCallback />;
  if (rootRoute === "dashboard") {
    return (
      <AuthGuard>
        <Dashboard />
      </AuthGuard>
    );
  }

  return <LandingPage />;
}

document.body.dataset.surface = resolveBodySurface({ path, isTauriRuntime });

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Suspense fallback={<BootstrapState message="กำลังเปิด FUNG…" />}>
      <RootRouter />
    </Suspense>
  </React.StrictMode>,
);
