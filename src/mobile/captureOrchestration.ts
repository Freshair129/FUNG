export type CaptureStartStage = "session" | "native-start" | "web-permission";

export type CaptureBackendResult<TMedia, TNative> = {
  backend: "android-native" | "web";
  recordingId: string;
  media: TMedia | null;
  native: TNative;
};

type SessionResult = { recordingId: string } | null;
type NativeResult = { available: boolean };

type CaptureBackendDependencies<TMedia, TNative extends NativeResult> = {
  createSession: () => Promise<SessionResult>;
  startNative: (recordingId: string) => Promise<TNative>;
  openWebAudio: () => Promise<TMedia>;
  createRecordingId: () => string;
};

export class CaptureStartError extends Error {
  readonly stage: CaptureStartStage;
  readonly detail: string;

  constructor(stage: CaptureStartStage, cause: unknown) {
    const detail = cause instanceof Error ? cause.message : String(cause);
    super(`capture ${stage} failed: ${detail}`);
    this.name = "CaptureStartError";
    this.stage = stage;
    this.detail = detail;
  }
}

export function resumeCaptureClock(startedAt: number | null, pausedAt: number | null, resumedAt: number): number | null {
  if (startedAt === null || pausedAt === null) return startedAt;
  return startedAt + Math.max(0, resumedAt - pausedAt);
}

export async function acquireCaptureBackend<TMedia, TNative extends NativeResult>(
  dependencies: CaptureBackendDependencies<TMedia, TNative>,
): Promise<CaptureBackendResult<TMedia, TNative>> {
  let session: SessionResult;
  try {
    session = await dependencies.createSession();
  } catch (error) {
    throw new CaptureStartError("session", error);
  }

  const recordingId = session?.recordingId ?? dependencies.createRecordingId();
  let native: TNative;
  try {
    native = await dependencies.startNative(recordingId);
  } catch (error) {
    throw new CaptureStartError("native-start", error);
  }

  if (native.available) {
    return { backend: "android-native", recordingId, media: null, native };
  }

  try {
    const media = await dependencies.openWebAudio();
    return { backend: "web", recordingId, media, native };
  } catch (error) {
    throw new CaptureStartError("web-permission", error);
  }
}
