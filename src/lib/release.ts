export const DESKTOP_RELEASE_VERSION = "0.1.0";

// Approximate installer size shown in the download CTA copy. Update this
// alongside DESKTOP_RELEASE_VERSION whenever a new build changes the size
// meaningfully, so the two facts stay in one place instead of drifting.
export const DESKTOP_RELEASE_SIZE_LABEL = "491 MB";

const DEFAULT_DESKTOP_RELEASE_DOWNLOAD_URL =
  "https://github.com/Freshair129/FUNG-Releases/releases/latest/download/FUNG-windows-x64-setup.exe";

// Overridable so forks/deploys aren't pinned to a personal GitHub account.
// Set VITE_RELEASE_DOWNLOAD_URL in the environment to point at your own
// release asset; falls back to the upstream FUNG-Releases URL otherwise.
export const DESKTOP_RELEASE_DOWNLOAD_URL =
  import.meta.env?.VITE_RELEASE_DOWNLOAD_URL ?? DEFAULT_DESKTOP_RELEASE_DOWNLOAD_URL;
