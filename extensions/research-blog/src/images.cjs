'use strict';
const vscode = require('vscode');
const path = require('node:path');
const fs = require('node:fs');
const { artDirection } = require('./image-style.cjs');
const { endpoint } = require('./ai-core.cjs');
const { scopeFor, trusted } = require('./writing.cjs');
const { articleKey, safePath } = require('./collections-core.cjs');
const core = require('./image-core.cjs');

function registerImages(context) {
  const pending = new Map();
  const options = uri => {
    const config = vscode.workspace.getConfiguration('researchBlog.images', uri);
    const ai = vscode.workspace.getConfiguration('researchBlog.ai', uri);
    return { baseUrl: config.get('baseUrl', '').trim() || ai.get('baseUrl', 'https://api.openai.com/v1'),
      model: config.get('model', '').trim(), style: artDirection(config.get('appearance', 'paper'), config.get('composition', 'concept'), config.get('styleInstructions', '')),
      useStyleReference: config.get('useStyleReference', false),
      size: config.get('size', '1536x1024'), quality: config.get('quality', 'auto'), timeoutSeconds: config.get('timeoutSeconds', 180), aiBase: ai.get('baseUrl', 'https://api.openai.com/v1'),
      aiModel: ai.get('model', '').trim(), aiTokenParameter: ai.get('tokenParameter'), preparePrompt: config.get('preparePrompt', false) };
  };
  const command = (name, action) => context.subscriptions.push(vscode.commands.registerCommand(name, async () => {
    try { trusted(); return await action(); }
    catch (error) { vscode.window.showErrorMessage(`Research Blog: ${error.message}`); }
  }));
  command('researchBlog.images.chooseStyle', async () => {
    const composition = await vscode.window.showQuickPick([
      { label: '概念示意图', description: '形象、分组和必要箭头，直观说明关系', value: 'concept' },
      { label: '正文配图', description: '围绕主题自由构图，少量标签', value: 'editorial' },
      { label: '系列封面', description: '主题鲜明，为标题保留空间', value: 'cover' },
    ], { title: '选择配图用途' });
    if (!composition) return;
    const appearance = await vscode.window.showQuickPick([
      { label: '石墨黑', description: '深色底、象牙白线条、少量陶土红', value: 'graphite' },
      { label: '暖纸白', description: '浅色底、石墨线条、少量深红', value: 'paper' },
    ], { title: '选择配图底色' });
    if (!appearance) return;
    const config = vscode.workspace.getConfiguration('researchBlog.images');
    await config.update('composition', composition.value, vscode.ConfigurationTarget.Global);
    await config.update('appearance', appearance.value, vscode.ConfigurationTarget.Global);
    vscode.window.showInformationMessage(`配图风格：${composition.label} · ${appearance.label}。`);
  });
  command('researchBlog.images.configureTokenPlan', async () => {
    const stored = await context.secrets.get(core.imageKeyName(core.TOKEN_PLAN_BASE));
    const validate = value => (!value.trim() && stored) || /^sk-sp-\S+$/.test(value.trim()) ? undefined : '请输入以 sk-sp- 开头的 Token Plan 套餐专属 Key。';
    const key = await vscode.window.showInputBox({ title: '百炼 Token Plan 生图 · Qwen Image 3.0 Pro', password: true, ignoreFocusOut: true,
      prompt: '填写套餐专属 Key；仅配置生图，文字模型保持现有设置。' + (stored ? ' 留空保留已存密钥。' : ''), validateInput: validate });
    if (key === undefined) return;
    if (validate(key)) throw new Error(validate(key));
    if (key.trim()) await context.secrets.store(core.imageKeyName(core.TOKEN_PLAN_BASE), key.trim());
    const config = vscode.workspace.getConfiguration('researchBlog.images');
    for (const [name, value] of Object.entries({ baseUrl: core.TOKEN_PLAN_BASE, model: 'qwen-image-3.0-pro', size: '1536x1024', quality: 'auto', timeoutSeconds: 300 })) {
      await config.update(name, value, vscode.ConfigurationTarget.Global);
    }
    vscode.window.showInformationMessage('已配置百炼 Token Plan · Qwen Image 3.0 Pro（1K 横图）。选中文章内容后，右键生成配图。');
  });
  command('researchBlog.images.configure', async () => {
    const current = options(vscode.window.activeTextEditor?.document.uri);
    const base = await vscode.window.showInputBox({ title: '生图 API 基础地址', value: current.baseUrl, ignoreFocusOut: true,
      prompt: '填写生图基础地址；百炼套餐可直接运行“配置百炼 Token Plan 生图”。',
      validateInput: value => { try { core.imageEndpoint(value); } catch (e) { return e.message; } } });
    if (!base) return;
    const model = await vscode.window.showInputBox({ title: '生图模型 ID', value: current.model, ignoreFocusOut: true, prompt: '填写服务提供方支持的图像模型 ID。', validateInput: value => value.trim() ? undefined : '请输入模型 ID。' });
    if (!model) return;
    const key = await vscode.window.showInputBox({ title: `生图 API Key (${core.imageEndpoint(base).origin})`, password: true, ignoreFocusOut: true, prompt: '保存在 SecretStorage；留空保留已存密钥，或复用同一接口的 AI 密钥。' });
    if (key === undefined) return;
    const config = vscode.workspace.getConfiguration('researchBlog.images');
    await config.update('baseUrl', base.trim(), vscode.ConfigurationTarget.Global);
    await config.update('model', model.trim(), vscode.ConfigurationTarget.Global);
    if (key.trim()) await context.secrets.store(core.imageKeyName(base.trim()), key.trim());
    vscode.window.showInformationMessage('生图配置已保存。选中文章中的文字，右键选择“AI 根据选中文字生成配图”。');
  });
  command('researchBlog.images.clearKey', async () => {
    await context.secrets.delete(core.imageKeyName(options(vscode.window.activeTextEditor?.document.uri).baseUrl));
    vscode.window.showInformationMessage('当前生图专用密钥已删除；同一接口的通用 AI 密钥仍可被复用。');
  });
  command('researchBlog.images.generate', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.uri.scheme !== 'file' || editor.document.languageId !== 'markdown' || editor.selection.isEmpty) throw new Error('请在文章中选中一段文字，再生成配图。');
    if (editor.selections?.length > 1) throw new Error('请保留一个选区后再生成配图。');
    const document = editor.document, id = document.uri.toString();
    if (pending.has(id)) throw new Error('这篇文章已有一张配图正在生成，可在进度通知中取消。');
    const scope = scopeFor(vscode.workspace.getWorkspaceFolder(document.uri));
    const key = articleKey(path.relative(scope.contentRoot, document.uri.fsPath).split(path.sep).join('/'));
    safePath(scope.contentRoot, key);
    const selection = document.getText(editor.selection), snapshot = document.getText(), version = document.version;
    const offset = core.insertionOffset(snapshot, document.offsetAt(editor.selection.start), document.offsetAt(editor.selection.end));
    const o = options(document.uri);
    core.makePrompt(selection, o.style);
    if (!o.model) {
      const action = await vscode.window.showInformationMessage('首次生图需要配置 API 地址、图像模型和密钥。', '配置生图接口');
      if (action) await vscode.commands.executeCommand('researchBlog.images.configure');
      return;
    }
    const url = core.imageEndpoint(o.baseUrl);
    o.key = await context.secrets.get(core.imageKeyName(o.baseUrl));
    if (!o.key) {
      let shared;
      try { if (endpoint(o.baseUrl).href === endpoint(o.aiBase).href) shared = endpoint(o.aiBase); }
      catch { /* An invalid completion endpoint must not block a separate image service. */ }
      if (shared) o.key = await context.secrets.get(`researchBlog.ai.key:${shared.origin}${shared.pathname}`);
    }
    if (!o.key && !['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)) throw new Error('请先运行“Research Blog: 配置 AI 生图接口”保存密钥。');
    const controller = new AbortController();
    pending.set(id, controller);
    let timedOut = false;
    const timeout = setTimeout(() => { timedOut = true; controller.abort(); }, Math.max(30, Math.min(600, o.timeoutSeconds)) * 1000);
    try {
      await vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: `生成博客配图 · ${url.hostname}`, cancellable: true }, async (progress, token) => {
        const subscription = token.onCancellationRequested(() => controller.abort());
        try {
          if (token.isCancellationRequested) controller.abort();
          if (controller.signal.aborted) return;
          if (o.preparePrompt && o.aiModel) {
            progress.report({ message: '根据选区整理画面构思…' });
            const aiUrl = endpoint(o.aiBase);
            let aiKey = await context.secrets.get(`researchBlog.ai.key:${aiUrl.origin}${aiUrl.pathname}`);
            if (!aiKey && aiUrl.href === endpoint(o.baseUrl).href) aiKey = o.key;
            if (!aiKey && !['localhost', '127.0.0.1', '[::1]'].includes(aiUrl.hostname)) throw new Error('请先设置文字模型 API Key，或关闭“先整理配图构思”。');
            o.prompt = await core.requestIllustrationPrompt({ baseUrl: o.aiBase, model: o.aiModel, key: aiKey, style: o.style, tokenParameter: o.aiTokenParameter }, selection, controller.signal);
          }
          if (controller.signal.aborted) return;
          progress.report({ message: '绘制正文示意图…' });
          if (o.useStyleReference && url.hostname === new URL(core.TOKEN_PLAN_BASE).hostname && context.asAbsolutePath) {
            const reference = context.asAbsolutePath('assets/editorial-reference.png');
            if (fs.existsSync(reference)) o.referenceImage = fs.readFileSync(reference);
          }
          const image = await core.requestImage(o, selection, controller.signal);
          if (controller.signal.aborted) return;
          const saved = core.saveImage(scope.contentRoot, document.uri.fsPath, image);
          const markdown = core.markdownImage(saved.relative, selection);
          let inserted = false, articleSaved = false;
          if (!document.isClosed && document.version === version && document.getText() === snapshot && offset !== null) {
            const edit = new vscode.WorkspaceEdit();
            edit.insert(document.uri, document.positionAt(offset), markdown);
            inserted = await vscode.workspace.applyEdit(edit);
            if (inserted) articleSaved = await document.save();
          }
          if (inserted) {
            void vscode.window.showInformationMessage(articleSaved ? '配图已保存，并插入到选中文段后。' : '图片已保存，链接已插入；文章尚未保存，请手动保存。', '查看图片')
              .then(action => action && vscode.commands.executeCommand('vscode.open', vscode.Uri.file(saved.file)));
          } else {
            void vscode.window.showInformationMessage('图片已保存。文章已改动或选区位于未闭合的代码/公式块中，请自行放置图片链接。', '复制图片链接', '查看图片').then(action => {
              if (action === '复制图片链接') return vscode.env.clipboard.writeText(markdown.trim());
              if (action === '查看图片') return vscode.commands.executeCommand('vscode.open', vscode.Uri.file(saved.file));
            });
          }
        } finally { subscription.dispose(); }
      });
    } catch (error) {
      if (timedOut) throw new Error('生图超时，请稍后重试，或在设置中增加生图超时时间。');
      if (!controller.signal.aborted) throw error;
    } finally { clearTimeout(timeout); pending.delete(id); }
  });
  context.subscriptions.push({ dispose() { for (const controller of pending.values()) controller.abort(); pending.clear(); } });
}
module.exports = { registerImages };
