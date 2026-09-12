'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const { completionContext, endpoint } = require('../src/ai-core.cjs');
test('inline provider gates opt-in/trust/scope, accepts suggestions and cancels stale requests', async () => {
  let provider, changeListener, requestCount = 0, resolveRequest;
  const root = path.resolve(__dirname, '../.test-work/provider');
  const settings = { enabled: false, model: 'configured-model', baseUrl: 'http://127.0.0.1:1234/v1' };
  const disposable = { dispose() {} };
  const vscode = {
    window: { createOutputChannel: () => ({ ...disposable, appendLine() {} }) },
    workspace: { isTrusted: true, getWorkspaceFolder: () => ({ uri: { fsPath: root } }),
      getConfiguration: section => ({ get: (name, fallback) => section === 'researchBlog.ai' ? settings[name] ?? fallback : fallback }),
      onDidChangeConfiguration: () => disposable,
      onDidChangeTextDocument: callback => { changeListener = callback; return disposable; } },
    commands: { registerCommand: () => disposable },
    languages: { registerInlineCompletionItemProvider: (_, p) => { provider = p; return disposable; } },
    InlineCompletionTriggerKind: { Invoke: 0 },
    Range: class { constructor(start, end) { this.start = start; this.end = end; } },
    InlineCompletionItem: class { constructor(insertText, range) { this.insertText = insertText; this.range = range; } },
  };
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.resolve(__dirname, '../src/ai.cjs'), 'utf8'), {
    module, setTimeout, clearTimeout, AbortController, Date,
    require: name => name === 'vscode' ? vscode : name === './ai-core.cjs' ? {
      completionContext, endpoint,
      requestCompletion: async (_options, _data, signal) => {
        requestCount++;
        return new Promise(resolve => { resolveRequest = resolve; signal.addEventListener('abort', () => resolve('stale'), { once: true }); });
      },
    } : require(name),
  });
  module.exports.registerAI({ subscriptions: [], secrets: { get: async () => undefined } });
  const document = { uri: { fsPath: path.join(root, 'site/content/posts/a/index.md') }, languageId: 'markdown', version: 1, getText: () => '$x = ', offsetAt: () => 5 };
  const token = { isCancellationRequested: false, onCancellationRequested: () => disposable };
  const invoke = () => provider.provideInlineCompletionItems(document, { line: 0, character: 5 }, { triggerKind: 0 }, token);
  assert.equal((await invoke()).length, 0); assert.equal(requestCount, 0);
  settings.enabled = true; vscode.workspace.isTrusted = false;
  assert.equal((await invoke()).length, 0); assert.equal(requestCount, 0);
  vscode.workspace.isTrusted = true;
  const pending = invoke(); await new Promise(r => setImmediate(r));
  assert.equal(requestCount, 1); document.version++; changeListener();
  assert.equal((await pending).length, 0);
  const next = invoke(); await new Promise(r => setImmediate(r)); resolveRequest('a^2');
  assert.equal((await next)[0].insertText, 'a^2');
  document.uri.fsPath = path.join(root, 'outside.md');
  assert.equal((await invoke()).length, 0); assert.equal(requestCount, 2);
});
