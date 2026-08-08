import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { LandingPage } from "./landing/LandingPage";
import { MobileApp } from "./mobile/MobileApp";
import { AuthCallback } from "./web/AuthCallback";
import { AuthGuard } from "./web/AuthGuard";
import { Dashboard } from "./web/Dashboard";
import "./styles.css";

const params = new URLSearchParams(window.location.search);
const path = window.location.pathname;
const isTauriRuntime = "__TAURI_INTERNALS__" in window;
const surface = params.get("surface");

function RootRouter() {
  // Tauri desktop runtime — always show the desktop app, no auth
  if (isTauriRuntime) {
    return <App />;
  }

  // OAuth callback — process tokens
  if (path === "/auth/callback") {
    return <AuthCallback />;
  }

  // /app with explicit surface — desktop/mobile app, no auth gate
  if (path === "/app" && (surface === "desktop" || surface === "mobile")) {
    const mobileViewport = window.matchMedia(
      "(pointer: coarse) and (max-width: 760px), (pointer: coarse) and (orientation: landscape) and (max-height: 760px)",
    ).matches;
    const ProductApp = surface === "mobile" || (!surface && mobileViewport) ? MobileApp : App;
    return <ProductApp />;
  }

  // /app without surface — web dashboard, requires auth
  if (path === "/app") {
    return (
      <AuthGuard>
        <Dashboard />
      </AuthGuard>
    );
  }

  // Landing page (default)
  return <LandingPage />;
}

document.body.dataset.surface =
  path === "/" || (!isTauriRuntime && path !== "/app" && path !== "/auth/callback")
    ? "landing"
    : "app";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RootRouter />
  </React.StrictMode>,
);
