/**
 * What a tile's job button is actually able to do.
 *
 * The focus tiles were written against a job vocabulary of thirteen types.
 * Three of them have an implementation behind them; the rest were labels on
 * buttons that filed a row nothing ran. Rather than let the UI keep offering
 * work the product cannot do, every job value resolves here into one of
 * three honest outcomes, and the backend refuses anything outside the
 * runnable set as a second line of defence.
 *
 * This mirrors `job_engine::JobKind` on the Rust side. If a kind is added
 * there, add it here; `runnable_job_types` exists so a UI can check the two
 * agree at runtime rather than trusting this file.
 */

/** Job types the engine has a handler for. Must match `JobKind::as_str`. */
export const RUNNABLE_JOB_TYPES = [
  "summary.generate",
  "transcript.retry",
  "graph.build",
  "speakers.diarize",
] as const;

export type RunnableJobType = (typeof RUNNABLE_JOB_TYPES)[number];

export type JobActionPlan =
  /** Queue this job type against the current project's active recording. */
  | { kind: "queue"; jobType: RunnableJobType; needsRecording: true }
  /** Not a queued job at all — the file-import + transcribe flow. */
  | { kind: "import" }
  /** Nothing behind this button. `reason` is shown to the user as-is. */
  | { kind: "unavailable"; reason: string };

/**
 * The three summary buttons all resolve to the same job.
 *
 * `summarize_and_export` produces the narrative, the timeline, and the
 * decisions/actions list in one run against one transcript — they are three
 * views of a single pass, not three pipelines. Queueing three jobs would run
 * the same work three times and, before the ids were made deterministic,
 * would have written three sets of rows.
 */
const SUMMARY_ALIASES = ["summary.recap", "summary.intent", "summary.actions"];

/**
 * Buttons with no implementation, and the reason each is dark.
 *
 * Stated per action rather than behind one generic "not available": a user
 * deciding whether to wait for a feature is owed the difference between
 * "needs a model FUNG does not ship" and "no one has built it".
 */
const UNAVAILABLE: Record<string, string> = {
  "capture.marker": "ยังไม่มีที่เก็บ marker — ต้องเพิ่มตารางก่อน",
  "speakers.lock": "ยังไม่มีการยืนยันผู้พูดแบบถาวร",
  "review.evidence": "ยังไม่มีการทำเครื่องหมายหลักฐาน",
  "summary.compare": "ยังเทียบสรุปข้ามครั้งไม่ได้",
  "export.render": "ส่งออกอัตโนมัติมากับสรุปการประชุมแล้ว",
  "export.queue": "ยังไม่มีคิวส่งออกแยก",
  "archive.project": "ใช้แผงสำรองข้อมูลแทน",
};

/** Resolves a tile action's job value into what can actually happen. */
export function resolveJobAction(value: string): JobActionPlan {
  const jobType = value.trim();

  if (jobType === "transcript.transcribe") {
    return { kind: "import" };
  }
  if (SUMMARY_ALIASES.includes(jobType)) {
    return { kind: "queue", jobType: "summary.generate", needsRecording: true };
  }
  if ((RUNNABLE_JOB_TYPES as readonly string[]).includes(jobType)) {
    return {
      kind: "queue",
      jobType: jobType as RunnableJobType,
      needsRecording: true,
    };
  }
  return {
    kind: "unavailable",
    reason: UNAVAILABLE[jobType] ?? `ยังไม่รองรับงานชนิด ${jobType}`,
  };
}

/**
 * Whether a tile button should be clickable at all.
 *
 * A disabled button with a reason beats one that looks live and does
 * nothing: the previous behaviour was a click, a row, and no feedback.
 */
export function isJobActionEnabled(
  value: string,
  hasRecording: boolean,
): boolean {
  const plan = resolveJobAction(value);
  if (plan.kind === "unavailable") return false;
  if (plan.kind === "import") return true;
  return hasRecording;
}

/**
 * Why a queueable action is still not clickable, or `null` when it is.
 *
 * Separated from {@link isJobActionEnabled} so the tooltip can distinguish
 * "this feature does not exist" from "this feature needs a recording you
 * have not made yet" — the second clears on its own, the first does not.
 */
export function jobActionBlockedReason(
  value: string,
  hasRecording: boolean,
): string | null {
  const plan = resolveJobAction(value);
  if (plan.kind === "unavailable") return plan.reason;
  if (plan.kind === "queue" && !hasRecording) {
    return "ต้องมีการบันทึกในโปรเจกต์นี้ก่อน";
  }
  return null;
}
