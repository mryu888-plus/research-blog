'use strict';
const vscode = require('vscode');
const path = require('node:path');
const { completionContext, endpoint, requestCompletion } = require('./ai-core.cjs');
function registerAI(context) {
  const output = vscode.window.createOutputChannel('Research Blog AI');
  context.subscriptions.push(output);
  const config = uri => vscode.workspace.getConfiguration('researchBlog.ai', uri || vscode.window.activeTextEditor?.document.uri);
  const keyName = base => `researchBlog.ai.key:${endpoint(base).origin}${endpoint(base).pathname}`;
  const options = uri => {
    const c = config(uri);
    return { enabled: c.get('enabled', false), baseUrl: c.get('baseUrl'), model: c.get('model', '').trim(),
      debounceMs: c.get('debounceMs', 600), contextChars: c.get('contextChars', 4000), maxTokens: c.get('maxTokens', 256), tokenParameter: c.get('tokenParameter') };
  };
  context.subscriptions.push(vscode.commands.registerCommand('researchBlog.ai.setKey', async () => {
    try {
      const base = options().baseUrl;
      const name = keyName(base);
      const key = await vscode.window.showInputBox({ title: `Research Blog API Key (${endpoint(base).origin})`, password: true, ignoreFocusOut: true, prompt: '密钥保存在 VS Code SecretStorage，不写入设置文件。' });
      if (key?.trim()) { await context.secrets.store(name, key.trim()); vscode.window.showInformationMessage('AI 密钥已保存。请填写模型 ID，并开启 researchBlog.ai.enabled。'); }
    } catch (error) { vscode.window.showErrorMessage(error.message); }
  }));
  context.subscriptions.push(vscode.commands.registerCommand('researchBlog.ai.clearKey', async () => {
    try { await context.secrets.delete(keyName(options().baseUrl)); vscode.window.showInformationMessage('当前 AI 接口的密钥已删除。'); }
    catch (error) { vscode.window.showErrorMessage(error.message); }
  }));
  context.subscriptions.push(vscode.commands.registerCommand('researchBlog.ai.complete', async () => {
    const o = options();
    if (!o.enabled || !o.model) { vscode.window.showInformationMessage('请在设置中填写 researchBlog.ai.model，并启用 researchBlog.ai.enabled。'); return; }
    const editor = vscode.window.activeTextEditor;
    if (!editor || !completionContext(editor.document.getText(), editor.document.offsetAt(editor.selection.active), editor.document.languageId)) {
      vscode.window.showInformationMessage('请把光标放在公式、围栏代码块或 Typst 文件中。'); return;
    }
    try {
      if (!['localhost', '127.0.0.1', '[::1]'].includes(endpoint(o.baseUrl).hostname) && !await context.secrets.get(keyName(o.baseUrl))) {
        vscode.window.showInformationMessage('请先运行 Research Blog: 设置 AI API Key。'); return;
      }
    } catch (error) { vscode.window.showErrorMessage(error.message); return; }
    await vscode.commands.executeCommand('editor.action.inlineSuggest.trigger');
  }));
  let pending;
  let lastErrorTime = 0;
  const cancel = () => pending?.abort();
  context.subscriptions.push({ dispose: cancel });
  context.subscriptions.push(vscode.workspace.onDidChangeConfiguration(cancel));
  context.subscriptions.push(vscode.workspace.onDidChangeTextDocument(cancel));
  context.subscriptions.push(vscode.languages.registerInlineCompletionItemProvider([
    { language: 'markdown', scheme: 'file' }, { language: 'typst', scheme: 'file' },
  ], {
    async provideInlineCompletionItems(document, position, inlineContext, token) {
      cancel();
      const o = options(document.uri);
      if (!o.enabled || !o.model || !vscode.workspace.isTrusted || token.isCancellationRequested) return [];
      const folder = vscode.workspace.getWorkspaceFolder(document.uri);
      if (!folder) return [];
      const root = path.resolve(folder.uri.fsPath, vscode.workspace.getConfiguration('researchBlog', document.uri).get('contentPath', 'site/content'));
      const relative = path.relative(root, document.uri.fsPath);
      if (relative.startsWith('..') || path.isAbsolute(relative)) return [];
      const data = completionContext(document.getText(), document.offsetAt(position), document.languageId, o.contextChars);
      if (!data) return [];
      const controller = new AbortController(); pending = controller;
      const cancellation = token.onCancellationRequested(() => controller.abort());
      const timeout = setTimeout(() => controller.abort(), 15000);
      const version = document.version;
      try {
        if (inlineContext.triggerKind !== vscode.InlineCompletionTriggerKind.Invoke) {
          await new Promise(resolve => {
            const finish = () => { clearTimeout(wait); controller.signal.removeEventListener('abort', finish); resolve(); };
            const wait = setTimeout(finish, o.debounceMs);
            controller.signal.addEventListener('abort', finish, { once: true });
          });
        }
        if (controller.signal.aborted) return [];
        o.key = await context.secrets.get(keyName(o.baseUrl));
        if (!o.key && !['localhost', '127.0.0.1', '[::1]'].includes(endpoint(o.baseUrl).hostname)) return [];
        const value = await requestCompletion(o, data, controller.signal);
        if (!value || token.isCancellationRequested || controller.signal.aborted || document.version !== version) return [];
        return [new vscode.InlineCompletionItem(value, new vscode.Range(position, position))];
      } catch (error) {
        if (!controller.signal.aborted && Date.now() - lastErrorTime > 30000) {
          // Never log document text, keys, or the server response body.
          output.appendLine(error.message); lastErrorTime = Date.now();
        }
        return [];
      } finally { clearTimeout(timeout); cancellation.dispose(); if (pending === controller) pending = undefined; }
    },
  }));
}
module.exports = { registerAI };
