// @req FR-101
import test from "node:test";
import assert from "node:assert/strict";
import {
  describeGapFill,
  describeRecovery,
  recoverRecording,
  scanForInterruptedRecordings,
} from "../src/lib/recoveryFlow.ts";

function fakeInvoke(handlers) {
  const calls = [];
  const invoke = async (command, args) => {
    calls.push({ command, args });
    const handler = handlers[command];
    if (handler instanceof Error) throw handler;
    return handler;
  };
  return { invoke, calls };
}

const noGaps = {
  chunksMissingTranscript: 0,
  chunksTranscribed: 0,
  stillMissing: 0,
  skippedReason: null,
};

test("a recovery that adopted nothing does not read like a rescue", () => {
  // The recording still gets closed out, but claiming recovered audio when
  // none was found would misreport what happened.
  const text = describeRecovery({
    adopted: {
      recordingId: "r1",
      adoptedChunks: 0,
      adoptedBytes: 0,
      unreadableFiles: 0,
      durationMs: 42_000,
    },
    transcript: noGaps,
  });
  assert.match(text, /ไม่มีไฟล์ค้างให้กู้คืน/);
  assert.doesNotMatch(text, /กู้คืนเสียง/);
});

test("recovered audio is reported with its size and resulting duration", () => {
  const text = describeRecovery({
    adopted: {
      recordingId: "r1",
      adoptedChunks: 6,
      adoptedBytes: 3 * 1024 * 1024,
      unreadableFiles: 0,
      durationMs: 90_000,
    },
    transcript: { chunksMissingTranscript: 6, chunksTranscribed: 6, stillMissing: 0, skippedReason: null },
  });
  assert.match(text, /กู้คืนเสียง 6 ช่วง/);
  assert.match(text, /3\.0 MB/);
  assert.match(text, /90 วินาที/);
});

test("unreadable files are named rather than folded into the success count", () => {
  const text = describeRecovery({
    adopted: {
      recordingId: "r1",
      adoptedChunks: 4,
      adoptedBytes: 1024,
      unreadableFiles: 2,
      durationMs: 20_000,
    },
    transcript: noGaps,
  });
  assert.match(text, /กู้คืนเสียง 4 ช่วง/);
  assert.match(text, /อ่านไม่ได้ 2 ไฟล์/);
  assert.match(text, /ยังอยู่ในโฟลเดอร์เดิม/);
});

test("recovery refuses to run without a recording id", async () => {
  const { invoke, calls } = fakeInvoke({ recovery_recover: {} });
  await assert.rejects(() => recoverRecording(invoke, ""), /missing_recording_id/);
  assert.equal(calls.length, 0);
});

test("the scan passes through the native report unchanged", async () => {
  const report = {
    interrupted: [
      {
        recordingId: "r1",
        projectId: "p1",
        status: "recording",
        knownChunks: 12,
        orphanFiles: ["a.wav", "b.wav"],
        missingFiles: 0,
      },
    ],
    staleJobsFailed: 3,
  };
  const { invoke, calls } = fakeInvoke({ recovery_scan: report });
  const result = await scanForInterruptedRecordings(invoke);
  assert.equal(result.interrupted.length, 1);
  assert.equal(result.interrupted[0].orphanFiles.length, 2);
  assert.equal(result.staleJobsFailed, 3);
  assert.equal(calls[0].command, "recovery_scan");
});

test("recovered audio with no words is reported, not glossed over", () => {
  // Adoption alone leaves a recording that is safe and unreadable at the same
  // time; the transcript result has to be stated alongside it.
  const text = describeRecovery({
    adopted: {
      recordingId: "r1",
      adoptedChunks: 5,
      adoptedBytes: 2048,
      unreadableFiles: 0,
      durationMs: 40_000,
    },
    transcript: {
      chunksMissingTranscript: 5,
      chunksTranscribed: 2,
      stillMissing: 3,
      skippedReason: null,
    },
  });
  assert.match(text, /กู้คืนเสียง 5 ช่วง/);
  assert.match(text, /ถอดความเพิ่ม 2 ช่วง/);
  assert.match(text, /ยังขาดอีก 3 ช่วง/);
});

test("a gap-fill that declined is never reported as a complete transcript", () => {
  // "Nothing to do" and "did not check" must not look alike.
  const declined = describeGapFill({
    chunksMissingTranscript: 0,
    chunksTranscribed: 0,
    stillMissing: 0,
    skippedReason: "too many segments to enumerate",
  });
  assert.match(declined, /ยังไม่ได้ตรวจ/);
  assert.doesNotMatch(declined, /ครบอยู่แล้ว/);

  assert.match(describeGapFill(noGaps), /ครบอยู่แล้ว/);
  assert.match(
    describeGapFill({ chunksMissingTranscript: 4, chunksTranscribed: 4, stillMissing: 0, skippedReason: null }),
    /ถอดความเพิ่ม 4 ช่วงจนครบ/,
  );
});
