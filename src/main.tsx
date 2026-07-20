import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { MobileApp } from "./mobile/MobileApp";
import "./styles.css";

const params = new URLSearchParams(window.location.search);
const forceMobile = params.get("surface") === "mobile";
const forceDesktop = params.get("surface") === "desktop";
const mobileViewport = window.matchMedia("(max-width: 760px)").matches;
const RootApp = forceMobile || (!forceDesktop && mobileViewport) ? MobileApp : App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RootApp />
  </React.StrictMode>,
);
