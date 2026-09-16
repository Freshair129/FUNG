/**
 * Recordings made in the browser by the web dashboard.
 *
 * The web build has no GenesisBlockDB and no Rust runtime behind it, so a
 * recording made on `/app` lives in this browser's own IndexedDB (per
 * origin, per profile) and nowhere else: nothing is uploaded, and it is not
 * visible from the desktop or another browser unless the user downloads the
 * file and moves it. The UI says so in as many words. The pure helpers here
 * (codec choice, file naming, formatting) are covered by
 * `tests/webRecordings.test.mjs`; the IndexedDB half only ever runs in a
 * browser.
 */

export type WebRecording = {
  id: string;
  /** ISO timestamp of when the recording was started. */
  createdAt: string;
  durationMs: number;
  /** The container/codec MediaRecorder actually produced, e.g. `audio/webm;codecs=opus`. */
  mimeType: string;
  bytes: number;
};

/**
 * Set once the recording has been handed to the desktop on this machine for
 * transcription (`useDesktopTranscription`): the desktop's job and the
 * recording it will write, so the transcript can be found again later.
 */
export type DesktopTranscription = {
  jobId: string;
  projectId: string;
  recordingId: string;
  sentAt: string;
};

export type StoredWebRecording = WebRecording & { blob: Blob; desktop?: DesktopTranscription };

const DB_NAME = "fung-web";
const DB_VERSION = 1;
const STORE = "recordings";

/**
 * Codecs to try, best first. Chrome/Edge/Firefox record WebM/Opus; Safari
 * (macOS 14.1+/iOS 17.1+) only offers MP4/AAC. The first one the browser's
 * `MediaRecorder.isTypeSupported` accepts wins; `null` means the browser
 * cannot record audio at all and the caller must say so instead of guessing.
 */
export const RECORDER_MIME_CANDIDATES: readonly string[] = [
  "audio/webm;codecs=opus",
  "audio/webm",
  "audio/mp4",
  "audio/ogg;codecs=opus",
];

export function pickRecorderMimeType(isTypeSupported: (mimeType: string) => boolean): string | null {
  for (const candidate of RECORDER_MIME_CANDIDATES) {
    if (isTypeSupported(candidate)) return candidate;
  }
  return null;
}

export function webRecordingExtension(mimeType: string): "webm" | "m4a" | "ogg" | "bin" {
  const container = mimeType.split(";")[0]?.trim().toLowerCase() ?? "";
  if (container === "audio/webm") return "webm";
  if (container === "audio/mp4") return "m4a";
  if (container === "audio/ogg") return "ogg";
  return "bin";
}

/**
 * `fung-web-2026-09-16-1203.webm` — sortable, no characters a filesystem
 * or a download prompt will mangle, and the local wall-clock time the user
 * remembers rather than UTC.
 */
export function webRecordingFileName(recording: Pick<WebRecording, "createdAt" | "mimeType">): string {
  const date = new Date(recording.createdAt);
  const pad = (value: number) => String(value).padStart(2, "0");
  const stamp = Number.isNaN(date.getTime())
    ? "unknown"
    : `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}`;
  return `fung-web-${stamp}.${webRecordingExtension(recording.mimeType)}`;
}

/** `m:ss`, or `h:mm:ss` once an hour is reached. */
export function formatDurationMs(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const mmss = `${hours > 0 ? String(minutes).padStart(2, "0") : minutes}:${String(seconds).padStart(2, "0")}`;
  return hours > 0 ? `${hours}:${mmss}` : mmss;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** True when this browser can both record audio and keep the result. */
export function webRecordingSupported(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof indexedDB !== "undefined" &&
    typeof MediaRecorder !== "undefined" &&
    Boolean(navigator.mediaDevices?.getUserMedia)
  );
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE)) {
        const store = db.createObjectStore(STORE, { keyPath: "id" });
        store.createIndex("createdAt", "createdAt");
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("indexedDB.open failed"));
    request.onblocked = () => reject(new Error("indexedDB.open blocked by another tab"));
  });
}

function requestToPromise<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("indexedDB request failed"));
  });
}

function transactionDone(transaction: IDBTransaction, what: string): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error(`indexedDB ${what} failed`));
    transaction.onabort = () => reject(transaction.error ?? new Error(`indexedDB ${what} aborted`));
  });
}

/** Newest first. */
export async function listWebRecordings(): Promise<StoredWebRecording[]> {
  const db = await openDatabase();
  try {
    const store = db.transaction(STORE, "readonly").objectStore(STORE);
    const rows = await requestToPromise(store.getAll() as IDBRequest<StoredWebRecording[]>);
    return rows.sort((a, b) => (a.createdAt < b.createdAt ? 1 : a.createdAt > b.createdAt ? -1 : 0));
  } finally {
    db.close();
  }
}

export async function saveWebRecording(recording: StoredWebRecording): Promise<void> {
  const db = await openDatabase();
  try {
    const transaction = db.transaction(STORE, "readwrite");
    transaction.objectStore(STORE).put(recording);
    await transactionDone(transaction, "write");
  } finally {
    db.close();
  }
}

/** Merges `patch` into the stored record; a missing id is a no-op. */
export async function updateWebRecording(
  id: string,
  patch: Partial<Omit<StoredWebRecording, "id">>,
): Promise<void> {
  const db = await openDatabase();
  try {
    const transaction = db.transaction(STORE, "readwrite");
    const store = transaction.objectStore(STORE);
    const existing = await requestToPromise(store.get(id) as IDBRequest<StoredWebRecording | undefined>);
    if (existing) store.put({ ...existing, ...patch, id });
    await transactionDone(transaction, "update");
  } finally {
    db.close();
  }
}

export async function deleteWebRecording(id: string): Promise<void> {
  const db = await openDatabase();
  try {
    const transaction = db.transaction(STORE, "readwrite");
    transaction.objectStore(STORE).delete(id);
    await transactionDone(transaction, "delete");
  } finally {
    db.close();
  }
}
