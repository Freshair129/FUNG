#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { createHash, randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { resolveRunStatus } from './workflow-run-status.mjs';

const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(scriptPath), '..');
const aiStudioRoot = path.resolve(repoRoot, '..', '..');
const runnerPath = path.join(repoRoot, 'scripts', 'run-workflow-acceptance.ps1');
const powershellPath = path.join(
  process.env.SystemRoot ?? 'C:\\Windows',
  'System32',
  'WindowsPowerShell',
  'v1.0',
  'powershell.exe',
);
const evidenceRoot = path.join(aiStudioRoot, 'verification', 'fung-workflow-mcp');
const fixturePath = path.join(
  aiStudioRoot,
  'verification',
  '2026-10-15-readiness-fixes',
  'FUNG',
  'fixtures',
  'standalone-audio-48k.wav',
);
const fixtureSha256 = '493E7CFAD303F6FF3032E0777B811317F48F6C08E74A23DEE779977F674B933F';
const modernVersion = '2026-07-28';
const protocolVersionMetaKey = 'io.modelcontextprotocol/protocolVersion';
const serverInfoMetaKey = 'io.modelcontextprotocol/serverInfo';
const supportedLegacyVersions = new Set(['2025-11-25']);
const toolTimeoutMs = 14 * 60 * 1000;
const maxLogChars = 128 * 1024;

let initialized = false;
let activeRun = null;
let inputBuffer = '';

const tools = [
  {
    name: 'fung.workflow.describe',
    title: 'Describe FUNG audio workflow acceptance',
    description:
      'Read the fixed local acceptance workflow and check its pinned synthetic audio fixture and staged CPU Whisper runtime. Does not open FUNG or use a microphone, GPU, Ollama, or user recordings.',
    inputSchema: { type: 'object', additionalProperties: false },
    outputSchema: {
      type: 'object',
      properties: {
        ready: { type: 'boolean' },
        workflow: { type: 'string' },
        fixture: { type: 'object' },
        runtime: { type: 'object' },
        resourcePolicy: { type: 'object' },
      },
      required: ['ready', 'workflow', 'fixture', 'runtime', 'resourcePolicy'],
    },
    annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  },
  {
    name: 'fung.workflow.run_acceptance',
    title: 'Run FUNG audio workflow acceptance',
    description:
      'Runs one end-to-end functional check on the pinned synthetic fixture: loopback import, real CPU transcription, transcript edit, SRT/VTT/WAV export, and Genesis database close/reopen. Transcript keyword accuracy is diagnostic only. Creates a new timestamped evidence folder and may take several minutes.',
    inputSchema: { type: 'object', additionalProperties: false },
    outputSchema: {
      type: 'object',
      properties: {
        status: { type: 'string', enum: ['PASS', 'FAIL'] },
        workflowResult: { type: 'object' },
        evidenceRoot: { type: 'string' },
        runReceipt: { type: 'string' },
        log: { type: 'string' },
      },
      required: ['status', 'evidenceRoot', 'runReceipt', 'log'],
    },
    annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: false },
  },
  {
    name: 'fung.workflow.list_runs',
    title: 'List recent FUNG workflow runs',
    description:
      'Lists recent local workflow receipts, including step timings and the diagnostic-only keyword metric. RUNNING receipts older than the 14-minute runner timeout plus a one-minute grace period are reported as STALE. Never reads transcript text or audio contents.',
    inputSchema: {
      type: 'object',
      properties: { limit: { type: 'integer', minimum: 1, maximum: 20, default: 10 } },
      additionalProperties: false,
    },
    outputSchema: {
      type: 'object',
      properties: { runs: { type: 'array', items: { type: 'object' } } },
      required: ['runs'],
    },
    annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: false },
  },
];

function writeMessage(message) {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}

function sendResult(id, result, modern = false) {
  writeMessage({
    jsonrpc: '2.0',
    id,
    result: modern
      ? { ...result, _meta: { [serverInfoMetaKey]: { name: 'fung-workflow-mcp', version: '0.1.0' } } }
      : result,
  });
}

function sendError(id, code, message, data) {
  writeMessage({
    jsonrpc: '2.0',
    id,
    error: { code, message, ...(data ? { data } : {}) },
  });
}

function sendProgress(token, progress, total, message, modern = false) {
  if (token === undefined || token === null) return;
  writeMessage({
    jsonrpc: '2.0',
    method: 'notifications/progress',
    params: {
      progressToken: token,
      progress,
      total,
      message,
      ...(modern ? { _meta: { [protocolVersionMetaKey]: modernVersion } } : {}),
    },
  });
}

function toolResult(value, isError = false, modern = false) {
  return {
    ...(modern ? { resultType: 'complete' } : {}),
    content: [{ type: 'text', text: JSON.stringify(value, null, 2) }],
    structuredContent: value,
    isError,
  };
}

function checkReadiness() {
  const runtimePython = path.join(repoRoot, '.venv-whisper', 'Scripts', 'python.exe');
  const runtimeManifest = path.join(repoRoot, '.venv-whisper', 'manifest.json');
  const fixtureExists = existsSync(fixturePath);
  let fixtureHashMatches = false;
  if (fixtureExists) {
    fixtureHashMatches = createHash('sha256').update(readFileSync(fixturePath)).digest('hex').toUpperCase() === fixtureSha256;
  }
  const runtimeExists = existsSync(runtimePython) && existsSync(runtimeManifest);
  let pinnedRuntimeAvailable = false;
  if (runtimeExists) {
    try {
      const manifest = JSON.parse(readFileSync(runtimeManifest, 'utf8'));
      pinnedRuntimeAvailable = Array.isArray(manifest.models) && manifest.models.some((model) =>
        model.name === 'large-v3-turbo' && model.revision === '0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf',
      );
    } catch {
      pinnedRuntimeAvailable = false;
    }
  }
  return {
    ready: fixtureExists && fixtureHashMatches && pinnedRuntimeAvailable && existsSync(runnerPath) && existsSync(powershellPath),
    workflow: 'loopback_import_transcribe_edit_export_reopen',
    fixture: {
      id: 'standalone-english-48k',
      exists: fixtureExists,
      sha256: fixtureSha256,
      verification: 'verified by the PowerShell runner immediately before execution',
    },
    runtime: {
      available: pinnedRuntimeAvailable,
      profile: 'CPU int8 large-v3-turbo',
      modelRevision: '0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf',
      python: runtimeExists ? runtimePython : null,
    },
    runner: { windowsPowerShellAvailable: existsSync(powershellPath) },
    resourcePolicy: {
      microphone: 'not used',
      userAudio: 'not read',
      gpuInference: 'not run',
      ollama: 'not touched',
      output: 'new unique folder under G:\\AI-Studio\\verification\\fung-workflow-mcp',
    },
  };
}

function readRecentRuns(limit) {
  if (!existsSync(evidenceRoot)) return { runs: [] };
  const runDirectories = readdirSync(evidenceRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && /^workflow-[A-Za-z0-9-]{8,80}$/.test(entry.name))
    .map((entry) => path.join(evidenceRoot, entry.name))
    .sort((left, right) => {
      const leftStat = Date.parse(readReceipt(left)?.startedAt ?? '') || 0;
      const rightStat = Date.parse(readReceipt(right)?.startedAt ?? '') || 0;
      return rightStat - leftStat;
    });
  const runs = runDirectories.slice(0, limit).map((directory) => {
    const receipt = readReceipt(directory);
    if (!receipt) return { runId: path.basename(directory), status: 'RECEIPT_UNREADABLE' };
    const result = receipt.workflowResult;
    return {
      runId: receipt.runId,
      status: resolveRunStatus(receipt, Date.now(), toolTimeoutMs + 60_000),
      sourceHead: receipt.sourceHead,
      startedAt: receipt.startedAt,
      durationMs: receipt.durationMs,
      transcriptionProfile: receipt.transcriptionProfile,
      computeType: receipt.computeType,
      whisperModelProfile: receipt.whisperModelProfile,
      qualityGate: receipt.qualityGate,
      timingsMs: result?.timingsMs ?? null,
      transcriptDiagnostic: result?.transcript
        ? {
            segmentCount: result.transcript.segmentCount,
            keywordMatch: result.transcript.keywordMatch,
            keywordCount: result.transcript.keywordCount,
            keywordScorePercent: result.transcript.keywordScorePercent,
          }
        : null,
      evidenceRoot: receipt.evidenceRoot,
    };
  });
  return { runs };
}

function readReceipt(directory) {
  const receiptPath = path.join(directory, 'run-receipt.json');
  try {
    return JSON.parse(readFileSync(receiptPath, 'utf8'));
  } catch {
    return null;
  }
}

function terminateProcessTree(pid) {
  if (process.platform === 'win32') {
    const killer = spawn('taskkill.exe', ['/PID', String(pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
    killer.unref();
  }
}

function runAcceptance(progressToken, requestId, modern) {
  if (activeRun) {
    throw new Error('A workflow acceptance is already running in this MCP server process.');
  }
  const readiness = checkReadiness();
  if (!readiness.ready) {
    throw new Error('Pinned fixture, staged runtime, or fixed runner is missing. Call fung.workflow.describe for details.');
  }

  const now = new Date().toISOString().replaceAll(':', '').replaceAll('.', '');
  const runId = `workflow-${now}-${randomUUID().slice(0, 8)}`;
  const runStartedAt = new Date().toISOString();
  const runRoot = path.join(evidenceRoot, runId);
  const receiptPath = path.join(runRoot, 'run-receipt.json');
  const logPath = path.join(runRoot, 'run.log');
  const child = spawn(
    powershellPath,
    ['-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', runnerPath, '-RunId', runId],
    { cwd: repoRoot, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] },
  );
  const run = { child, cancelled: false, settled: false, requestId, modern };
  activeRun = run;
  sendProgress(progressToken, 0, 3, 'Starting the fixed CPU acceptance runner.', modern);

  return new Promise((resolve, reject) => {
    let stdout = '';
    let stderr = '';
    let progressNumber = 0;
    let settled = false;
    const rejectOnce = (error) => {
      if (settled) return;
      settled = true;
      reject(error);
    };
    const resolveOnce = (value) => {
      if (settled) return;
      settled = true;
      resolve(value);
    };
    const timer = setTimeout(() => {
      run.cancelled = true;
      terminateProcessTree(child.pid);
      rejectOnce(new Error(`Workflow acceptance timed out after ${toolTimeoutMs / 60000} minutes; inspect ${runRoot}.`));
    }, toolTimeoutMs);

    const appendBounded = (current, chunk) => `${current}${chunk}`.slice(-maxLogChars);
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      stdout = appendBounded(stdout, chunk);
    });
    child.stderr.on('data', (chunk) => {
      stderr = appendBounded(stderr, chunk);
      for (const line of chunk.split(/\r?\n/)) {
        const marker = line.match(/^FUNG_WORKFLOW_PROGRESS:(\{.*\})$/);
        if (!marker) continue;
        try {
          const event = JSON.parse(marker[1]);
          progressNumber += 1;
          sendProgress(progressToken, Math.min(progressNumber, 2), 3, event.stage, modern);
        } catch {
          // Progress hints must never affect the workflow result.
        }
      }
    });
    child.once('error', (error) => {
      clearTimeout(timer);
      run.settled = true;
      if (activeRun === run) activeRun = null;
      rejectOnce(new Error(`Could not start the fixed PowerShell runner: ${error.message}`));
    });
    child.once('close', (code, signal) => {
      clearTimeout(timer);
      run.settled = true;
      if (activeRun === run) activeRun = null;
      if (run.cancelled) {
        rejectOnce(new Error(`Workflow acceptance was cancelled; inspect ${runRoot}.`));
        return;
      }
      try {
        const receipt = JSON.parse(readFileSync(receiptPath, 'utf8'));
        const result = receipt.workflowResult;
        const pass = code === 0 && receipt.status === 'PASS' && result?.status === 'PASS';
        sendProgress(progressToken, 3, 3, pass ? 'Workflow acceptance passed.' : 'Workflow acceptance failed.', modern);
        resolveOnce({
          status: pass ? 'PASS' : 'FAIL',
          runId,
          sourceHead: receipt.sourceHead,
          qualityGate: 'diagnostic_only',
          workflowResult: result ?? null,
          evidenceRoot: runRoot,
          runReceipt: receiptPath,
          log: logPath,
          ...(pass
            ? {}
            : {
                exitCode: code,
                signal,
                errorSummary: receipt.error ?? null,
                logsCapturedChars: stdout.length + stderr.length,
              }),
        });
      } catch {
        const diagnostic = stderr
          .split(/\r?\n/)
          .filter((line) => line && !line.startsWith('FUNG_WORKFLOW_PROGRESS:'))
          .slice(-8)
          .join('\n')
          .slice(-3000);
        try {
          mkdirSync(runRoot, { recursive: true });
          const failureReceipt = {
            runId,
            status: 'FAIL',
            sourceHead: null,
            startedAt: runStartedAt,
            finishedAt: new Date().toISOString(),
            error: diagnostic || `runner exit code ${code ?? 'unknown'}${signal ? ` (${signal})` : ''}`,
            evidenceRoot: runRoot,
            log: logPath,
          };
          if (!existsSync(logPath)) {
            writeFileSync(logPath, diagnostic || 'Runner exited before producing a receipt.\n', { flag: 'wx' });
          }
          if (!existsSync(receiptPath)) {
            writeFileSync(receiptPath, `${JSON.stringify(failureReceipt, null, 2)}\n`, { flag: 'wx' });
          }
        } catch {
          // Do not replace the original failure with a secondary evidence-write error.
        }
        rejectOnce(
          new Error(
            `Acceptance ended with exit code ${code ?? 'unknown'}${signal ? ` (${signal})` : ''}; no valid runner receipt was produced. ${diagnostic || `Inspect ${runRoot}.`}`,
          ),
        );
      }
    });
  });
}

async function callTool(name, args, progressToken, requestId, modern) {
  if (args === null || typeof args !== 'object' || Array.isArray(args)) {
    throw new Error('Tool arguments must be a JSON object.');
  }
  switch (name) {
    case 'fung.workflow.describe': {
      if (Object.keys(args).length !== 0) throw new Error('This tool accepts no arguments.');
      return checkReadiness();
    }
    case 'fung.workflow.run_acceptance': {
      if (Object.keys(args).length !== 0) throw new Error('This tool accepts no arguments.');
      return runAcceptance(progressToken, requestId, modern);
    }
    case 'fung.workflow.list_runs': {
      const limit = args.limit ?? 10;
      if (!Number.isInteger(limit) || limit < 1 || limit > 20) {
        throw new Error('limit must be an integer from 1 to 20.');
      }
      if (Object.keys(args).some((key) => key !== 'limit')) throw new Error('Unsupported argument.');
      return readRecentRuns(limit);
    }
    default:
      throw new Error(`Unknown tool: ${name}`);
  }
}

async function handleMessage(message) {
  if (!message || typeof message !== 'object' || message.jsonrpc !== '2.0') return;
  const id = message.id;
  const isRequest = id !== undefined;
  const params = message.params ?? {};
  const protocolVersion = params._meta?.[protocolVersionMetaKey];
  const modern = protocolVersion === modernVersion;
  try {
    if (protocolVersion && !modern && message.method !== 'initialize') {
      sendError(
        id,
        -32022,
        'Unsupported protocol version',
        { supported: [modernVersion, '2025-11-25'], requested: protocolVersion },
      );
      return;
    }
    switch (message.method) {
      case 'server/discover':
        if (!isRequest) return;
        if (protocolVersion !== modernVersion) {
          sendError(id, -32022, 'Unsupported protocol version', { supported: [modernVersion, '2025-11-25'], requested: protocolVersion ?? null });
          return;
        }
        sendResult(
          id,
          {
            resultType: 'complete',
            supportedVersions: [modernVersion, '2025-11-25'],
            capabilities: { tools: {} },
            instructions:
              'This server only runs or reports the pinned synthetic CPU functional acceptance workflow. Keyword accuracy is diagnostic; a PASS means the workflow produced and reopened its outputs.',
            ttlMs: 300000,
            cacheScope: 'public',
          },
          true,
        );
        return;
      case 'initialize': {
        if (!isRequest) return;
        const requestedVersion = params.protocolVersion;
        const protocolVersion = supportedLegacyVersions.has(requestedVersion) ? requestedVersion : '2025-11-25';
        initialized = true;
        sendResult(id, {
          protocolVersion,
          capabilities: { tools: { listChanged: false } },
          serverInfo: { name: 'fung-workflow-mcp', version: '0.1.0' },
          instructions:
            'This server only runs or reports the pinned synthetic CPU functional acceptance workflow. Keyword accuracy is diagnostic; a PASS means the workflow produced and reopened its outputs.',
        });
        return;
      }
      case 'notifications/initialized':
        initialized = true;
        return;
      case 'notifications/cancelled': {
        const requestId = params.requestId;
        if (activeRun && requestId === activeRun.requestId) {
          activeRun.cancelled = true;
          terminateProcessTree(activeRun.child.pid);
        }
        return;
      }
      case 'ping':
        if (isRequest) sendResult(id, {});
        return;
      case 'tools/list':
        if (!isRequest) return;
        if (!modern && !initialized) throw Object.assign(new Error('Server must be initialized first.'), { code: -32002 });
        sendResult(
          id,
          modern
            ? { resultType: 'complete', tools, ttlMs: 300000, cacheScope: 'public' }
            : { tools },
          modern,
        );
        return;
      case 'tools/call': {
        if (!isRequest) return;
        if (!modern && !initialized) throw Object.assign(new Error('Server must be initialized first.'), { code: -32002 });
        const toolName = params.name;
        const progressToken = params._meta?.progressToken;
        try {
          const value = await callTool(toolName, params.arguments ?? {}, progressToken, id, modern);
          sendResult(id, toolResult(value, value.status === 'FAIL', modern), modern);
        } catch (error) {
          sendResult(id, toolResult({ error: error.message }, true, modern), modern);
        }
        return;
      }
      default:
        if (isRequest) sendError(id, -32601, `Method not found: ${String(message.method)}`);
    }
  } catch (error) {
    if (isRequest) sendError(id, error.code ?? -32603, error.message ?? 'Internal error');
  }
}

const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
input.on('line', (line) => {
  inputBuffer = line;
  try {
    const message = JSON.parse(inputBuffer);
    void handleMessage(message);
  } catch {
    sendError(null, -32700, 'Parse error');
  }
});

input.on('close', () => {
  if (activeRun && !activeRun.settled) {
    activeRun.cancelled = true;
    terminateProcessTree(activeRun.child.pid);
  }
});
