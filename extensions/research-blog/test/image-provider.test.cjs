'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');
const core = require('../src/image-core.cjs');
const library = require('../src/collections-core.cjs');
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=', 'base64');
function harness(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'blog-image-command-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const contentRoot = path.join(root, 'site/content'); fs.mkdirSync(contentRoot, { recursive: true });
  const article = library.createArticle(contentRoot, '选区配图');
  let text = fs.readFileSync(article.file, 'utf8');
  const start = text.indexOf('在这里'), end = start + '在这里开始写作。'.length;
  const commands = {}, info = [], errors = [], requested = [], keyReads = [], updates = [], planned = [];
  const aiSettings = {}, secrets = new Map();
  let finish, cancelProgress, edited = 0, saved = 0;
  const disposable = { dispose() {} }, settings = { baseUrl: 'http://127.0.0.1:1234/v1', model: 'fixture-model' };
  const uri = { scheme: 'file', fsPath: article.file, toString: () => article.file };
  const document = { uri, languageId: 'markdown', version: 1, isClosed: false,
    getText: range => range ? text.slice(range.start, range.end) : text,
    offsetAt: value => value, positionAt: value => value,
    save: async () => { saved++; fs.writeFileSync(article.file, text); return true; } };
  const selection = { start, end, isEmpty: false };
  const editor = { document, selection, selections: [selection] };
  const vscode = {
    window: { activeTextEditor: editor, showErrorMessage: message => errors.push(message),
      showInformationMessage: async message => { info.push(message); },
      withProgress: async (_options, action) => action({ report() {} }, { onCancellationRequested: callback => { cancelProgress = callback; return disposable; } }) },
    workspace: { isTrusted: true, getWorkspaceFolder: () => ({ uri: { fsPath: root } }),
      getConfiguration: section => ({ get: (name, fallback) => (section === 'researchBlog.images' ? settings : aiSettings)[name] ?? fallback,
        update: async (name, value, target) => { updates.push({ section, name, value, target }); (section === 'researchBlog.images' ? settings : aiSettings)[name] = value; } }),
      applyEdit: async edit => { edited++; text = text.slice(0, edit.offset) + edit.text + text.slice(edit.offset); document.version++; return true; } },
    commands: { registerCommand: (name, handler) => { commands[name] = handler; return disposable; }, executeCommand: async () => {} },
    WorkspaceEdit: class { insert(_uri, offset, value) { this.offset = offset; this.text = value; } },
    ProgressLocation: { Notification: 1 }, ConfigurationTarget: { Global: 1 }, Uri: { file: fsPath => ({ fsPath }) },
  };
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.resolve(__dirname, '../src/images.cjs'), 'utf8'), {
    module, setTimeout, clearTimeout, AbortController, URL,
    require: name => name === 'vscode' ? vscode : name === './writing.cjs' ? {
      scopeFor: () => ({ contentRoot }), trusted: () => { if (!vscode.workspace.isTrusted) throw new Error('untrusted'); },
    } : name === './image-core.cjs' ? { ...core, requestIllustrationPrompt: async (options, selection) => { planned.push({ options, selection }); return 'Flat connected shapes.'; }, requestImage: async (options, selection, signal) => {
      requested.push({ options, selection });
      return new Promise(resolve => { finish = () => resolve({ buffer: PNG }); signal.addEventListener('abort', finish, { once: true }); });
    } } : require(name.startsWith('.') ? path.resolve(__dirname, '../src', name) : name),
  });
  module.exports.registerImages({ subscriptions: [], secrets: { get: async name => { keyReads.push(name); return secrets.get(name); }, store: async (name, value) => secrets.set(name, value) } });
  return { commands, document, editor, vscode, info, errors, requested, keyReads, article, settings, aiSettings, secrets, updates, planned,
    finish: () => finish(), cancel: () => cancelProgress(), edit: () => { text += '\n写作仍在继续'; document.version++; },
    stats: () => ({ edited, saved, text }), assets: () => fs.existsSync(path.join(path.dirname(article.file), 'assets')) ? fs.readdirSync(path.join(path.dirname(article.file), 'assets')) : [],
  };
}
test('image command inserts and saves after the selected paragraph, preserving all source text', async t => {
  const h = harness(t), original = h.stats().text;
  const pending = h.commands['researchBlog.images.generate'](); await new Promise(r => setImmediate(r)); h.finish(); await pending;
  assert.equal(h.requested.length, 1); assert.equal(h.requested[0].selection, '在这里开始写作。');
  assert.equal(h.stats().edited, 1); assert.equal(h.stats().saved, 1); assert.equal(h.assets().length, 1);
  assert(h.stats().text.includes('在这里开始写作。\n\n!['));
  assert.equal(h.stats().text.replace(/\n\n!\[[^\]]*\]\(assets\/[^)]+\)\n\n/, ''), original);
  assert.equal(h.errors.length, 0);
});
test('Token Plan configuration and generation preserve SiliconFlow text settings and isolate both credentials', async t => {
  const h = harness(t);
  Object.assign(h.aiSettings, { baseUrl: 'https://api.siliconflow.cn/v1', model: 'zai-org/GLM-5.3', tokenParameter: 'max_tokens' });
  h.secrets.set('researchBlog.ai.key:https://api.siliconflow.cn/v1/chat/completions', 'sk-text-fixture');
  const originalAI = { ...h.aiSettings };
  h.vscode.window.showInputBox = async options => { assert.equal(options.password, true); assert.equal(options.validateInput('sk-wrong'), '请输入以 sk-sp- 开头的 Token Plan 套餐专属 Key。'); return 'sk-sp-image-fixture'; };
  await h.commands['researchBlog.images.configureTokenPlan']();
  assert.deepEqual(h.aiSettings, originalAI);
  assert(h.updates.every(update => update.section === 'researchBlog.images'));
  assert.equal(h.settings.model, 'qwen-image-3.0-pro');
  const pending = h.commands['researchBlog.images.generate'](); await new Promise(r => setImmediate(r)); h.finish(); await pending;
  assert.equal(h.planned[0].options.key, 'sk-text-fixture');
  assert.equal(h.planned[0].options.model, originalAI.model);
  assert.equal(h.requested[0].options.key, 'sk-sp-image-fixture');
  assert.equal(h.requested[0].options.baseUrl, core.TOKEN_PLAN_BASE);
  assert.equal(h.requested[0].options.prompt, 'Flat connected shapes.');
  assert.equal(h.stats().saved, 1); assert.equal(h.errors.length, 0);
});
test('cancelling Token Plan configuration leaves existing image settings and credentials untouched', async t => {
  const h = harness(t), original = { ...h.settings };
  h.vscode.window.showInputBox = async () => undefined;
  await h.commands['researchBlog.images.configureTokenPlan']();
  assert.deepEqual(h.settings, original); assert.equal(h.updates.length, 0); assert.equal(h.secrets.size, 0);
});
test('editing while image generation runs saves the image without inserting stale text', async t => {
  const h = harness(t);
  const pending = h.commands['researchBlog.images.generate'](); await new Promise(r => setImmediate(r));
  h.edit(); h.finish(); await pending;
  assert.equal(h.stats().edited, 0); assert.equal(h.assets().length, 1);
  assert(h.stats().text.endsWith('写作仍在继续')); assert(h.info.some(message => message.includes('文章已改动')));
});
test('cancelled and duplicate requests do not write assets or launch extra requests', async t => {
  const h = harness(t);
  const pending = h.commands['researchBlog.images.generate'](); await new Promise(r => setImmediate(r));
  await h.commands['researchBlog.images.generate']();
  assert.equal(h.requested.length, 1); assert(h.errors.some(message => message.includes('正在生成')));
  h.cancel(); await pending;
  assert.equal(h.assets().length, 0); assert.equal(h.stats().edited, 0);
});
test('trust, selection and article scope are checked before provider calls or secret access', async t => {
  const h = harness(t);
  h.vscode.workspace.isTrusted = false; await h.commands['researchBlog.images.generate']();
  h.vscode.workspace.isTrusted = true; h.editor.selection.isEmpty = true; await h.commands['researchBlog.images.generate']();
  h.editor.selection.isEmpty = false; h.document.uri.fsPath = path.join(path.dirname(h.article.file), '..', 'outside.md');
  await h.commands['researchBlog.images.generate']();
  assert.equal(h.requested.length, 0); assert.equal(h.keyReads.length, 0); assert.equal(h.errors.length, 3);
});
