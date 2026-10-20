import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { resolveRunStatus } from '../scripts/workflow-run-status.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const serverPath = path.join(repoRoot, 'scripts', 'fung-workflow-mcp.mjs');
const modernVersion = '2026-07-28';
const protocolVersionMetaKey = 'io.modelcontextprotocol/protocolVersion';

test('stale runner receipts are distinguished from active runs', () => {
  const now = Date.parse('2026-10-20T10:30:00.000Z');
  const timeout = 15 * 60 * 1000;
  assert.equal(resolveRunStatus({ status: 'RUNNING', startedAt: '2026-10-20T10:14:59.999Z' }, now, timeout), 'STALE');
  assert.equal(resolveRunStatus({ status: 'RUNNING', startedAt: '2026-10-20T10:15:00.000Z' }, now, timeout), 'RUNNING');
  assert.equal(resolveRunStatus({ status: 'PASS', startedAt: '2026-10-20T09:00:00.000Z' }, now, timeout), 'PASS');
  assert.equal(resolveRunStatus({ status: 'RUNNING', startedAt: 'invalid' }, now, timeout), 'RUNNING');
});

function startClient() {
  const child = spawn(process.execPath, [serverPath], {
    cwd: repoRoot,
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true,
  });
  const pending = new Map();
  const lines = createInterface({ input: child.stdout, crlfDelay: Infinity });
  let nextId = 1;
  let stderr = '';
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (chunk) => { stderr += chunk; });
  lines.on('line', (line) => {
    const message = JSON.parse(line);
    if (message.id === undefined || !pending.has(message.id)) return;
    const { resolve, reject } = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) reject(new Error(JSON.stringify(message.error)));
    else resolve(message.result);
  });

  return {
    child,
    get stderr() { return stderr; },
    request(method, params = {}) {
      const id = nextId++;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
      });
    },
    notify(method, params = {}) {
      child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', method, params })}\n`);
    },
    close() {
      child.stdin.end();
      child.kill();
    },
  };
}

test('modern MCP discovery exposes only the fixed local workflow tools', async (t) => {
  const client = startClient();
  t.after(() => client.close());
  const meta = {
    [protocolVersionMetaKey]: modernVersion,
    'io.modelcontextprotocol/clientInfo': { name: 'fung-workflow-test', version: '1.0.0' },
    'io.modelcontextprotocol/clientCapabilities': {},
  };

  const discovery = await client.request('server/discover', { _meta: meta });
  assert.deepEqual(discovery.supportedVersions, [modernVersion, '2025-11-25']);
  assert.equal(discovery.capabilities.tools !== undefined, true);
  assert.equal(discovery._meta['io.modelcontextprotocol/serverInfo'].name, 'fung-workflow-mcp');

  const catalog = await client.request('tools/list', { _meta: meta });
  assert.equal(catalog.resultType, 'complete');
  assert.deepEqual(catalog.tools.map((tool) => tool.name), [
    'fung.workflow.describe',
    'fung.workflow.run_acceptance',
    'fung.workflow.list_runs',
  ]);
  const runTool = catalog.tools.find((tool) => tool.name === 'fung.workflow.run_acceptance');
  assert.equal(runTool.annotations.readOnlyHint, false);
  assert.deepEqual(runTool.inputSchema.required ?? [], []);

  const described = await client.request('tools/call', {
    name: 'fung.workflow.describe',
    arguments: {},
    _meta: meta,
  });
  assert.equal(described.isError, false);
  assert.equal(described.structuredContent.ready, true);
  assert.equal(described.structuredContent.workflow, 'loopback_import_transcribe_edit_export_reopen');
  assert.equal(described.structuredContent.runtime.modelRevision, '0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf');
  assert.equal(described.structuredContent.resourcePolicy.microphone, 'not used');
  assert.equal(client.stderr, '');
});

test('legacy MCP clients can initialize and call the same bounded tools', async (t) => {
  const client = startClient();
  t.after(() => client.close());

  const initialized = await client.request('initialize', {
    protocolVersion: '2025-11-25',
    capabilities: {},
    clientInfo: { name: 'fung-workflow-test', version: '1.0.0' },
  });
  assert.equal(initialized.protocolVersion, '2025-11-25');
  client.notify('notifications/initialized');

  const catalog = await client.request('tools/list');
  assert.deepEqual(catalog.tools.map((tool) => tool.name), [
    'fung.workflow.describe',
    'fung.workflow.run_acceptance',
    'fung.workflow.list_runs',
  ]);
  const described = await client.request('tools/call', {
    name: 'fung.workflow.describe',
    arguments: {},
  });
  assert.equal(described.isError, false);
  assert.equal(described.structuredContent.workflow, 'loopback_import_transcribe_edit_export_reopen');
  const runs = await client.request('tools/call', {
    name: 'fung.workflow.list_runs',
    arguments: { limit: 3 },
  });
  assert.equal(runs.isError, false);
  assert.ok(Array.isArray(runs.structuredContent.runs));
  assert.equal(runs.structuredContent.runs.length <= 3, true);
  assert.equal(runs.structuredContent.runs.some((run) => 'transcriptText' in run), false);
  assert.equal(client.stderr, '');
});
