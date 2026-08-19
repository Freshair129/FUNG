/**
 * The shape of a recording's post-meeting summaries.
 *
 * Kept out of `tauri.ts` because that module touches `window` at import
 * time, which makes it unloadable outside a browser — and the response shape
 * is exactly the thing worth checking against the Rust struct in a test.
 * `tauri.ts` re-exports these so callers see no difference.
 */

export type SummaryRow = {
  id: string;
  kind: string;
  content: string;
  evidenceCount: number;
  createdAt: string;
  /** The recording this summary describes, resolved through its model run. */
  recordingId: string;
  /** A newer summary of the same kind exists for this recording. */
  superseded: boolean;
};

/**
 * One recording's summaries and what was left out of them.
 *
 * The counts are not decoration. This read used to return every summary in
 * the project, so a user now seeing fewer rows is owed the reason, and an
 * `unattributable` row is a broken write that no recording-scoped read can
 * ever surface.
 */
export type MeetingSummaries = {
  /** Newest first; within a kind, only the first is not superseded. */
  rows: SummaryRow[];
  /** Summaries in the same project belonging to a different recording. */
  otherRecordings: number;
  /**
   * Summaries whose model run could not be found, so no recording can be
   * established for them. Only meaningful when `attributionComplete`.
   */
  unattributable: number;
  /**
   * False when the model-run lookup hit the storage engine's row ceiling, so
   * `unattributable` may include summaries whose recording simply was not
   * read. Do not present those as broken writes when this is false.
   */
  attributionComplete: boolean;
};

export const EMPTY_MEETING_SUMMARIES: MeetingSummaries = {
  rows: [],
  otherRecordings: 0,
  unattributable: 0,
  attributionComplete: true,
};
