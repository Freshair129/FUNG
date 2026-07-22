import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { LandingPage } from "./landing/LandingPage";
import { MobileApp } from "./mobile/MobileApp";
import "./styles.css";

const params = new URLSearchParams(window.location.search);
const isTauriRuntime = "__TAURI_INTERNALS__" in window;
const isLandingRoute = !isTauriRuntime && window.location.pathname === "/";
const forceMobile = params.get("surface") === "mobile";
const forceDesktop = params.get("surface") === "desktop";
const mobileViewport = window.matchMedia(
  "(pointer: coarse) and (max-width: 760px), (pointer: coarse) and (orientation: landscape) and (max-height: 760px)",
).matches;
const ProductApp = forceMobile || (!forceDesktop && mobileViewport) ? MobileApp : App;
const RootApp = isLandingRoute ? LandingPage : ProductApp;

document.body.dataset.surface = isLandingRoute ? "landing" : "app";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RootApp />
  </React.StrictMode>,
);
