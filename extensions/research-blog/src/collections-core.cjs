'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { randomUUID, createHash } = require('node:crypto');
const toml = require('smol-toml');

function inside(root, target) {
  const relative = path.relative(path.resolve(root), path.resolve(target));
  return relative === '' || (!relative.startsWith('..' + path.sep) && relative !== '..' && !path.isAbsolute(relative));
}
function safePath(root, ...parts) {
  root = path.resolve(root);
  const target = path.resolve(root, ...parts);
  if (!inside(root, target)) throw new Error('路径必须位于博客目录内。');
  let current = root;
  for (const part of ['', ...path.relative(root, target).split(path.sep).filter(Boolean)]) {
    current = part ? path.join(current, part) : current;
    try { if (fs.lstatSync(current).isSymbolicLink()) throw new Error('博客写作目录不能经过符号链接或目录联接。'); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  return target;
}
function titleText(value) {
  if (typeof value !== 'string' || !value.trim() || value.trim().length > 120 || /[\x00-\x1f]/.test(value)) throw new Error('请输入 1–120 个字符的标题。');
  return value.trim();
}
function articleKey(value) {
  if (typeof value !== 'string' || !/^posts\/[^/\\]+\/index\.md$/.test(value) || value.split('/').some(p => p === '.' || p === '..') || /[:\x00-\x1f]/.test(value)) throw new Error('文章必须是 posts/文章目录/index.md。');
  return value;
}
function validateCatalog(data) {
  if (!data || data.version !== 1 || !Array.isArray(data.series)) throw new Error('系列目录格式错误：需要 version: 1 和 series 数组。');
  const ids = new Set(), articles = new Set(), seriesTitles = new Set();
  for (const series of data.series) {
    if (!series || !Array.isArray(series.groups) || (series.articles !== undefined && !Array.isArray(series.articles))) throw new Error('系列需要 groups 数组，articles 如有填写也必须是数组。');
    const title = titleText(series.title);
    if (seriesTitles.has(title)) throw new Error('系列标题不能重复。');
    seriesTitles.add(title);
    const groupTitles = new Set();
    for (const entity of [series, ...series.groups]) {
      if (!entity || typeof entity.id !== 'string' || !/^[a-z][a-z0-9-]{1,79}$/.test(entity.id) || ids.has(entity.id)) throw new Error('系列和组需要唯一的英文 ID。');
      ids.add(entity.id); titleText(entity.title);
      if (entity.description !== undefined && (typeof entity.description !== 'string' || entity.description.length > 2000)) throw new Error('简介需为不超过 2000 字的文字。');
    }
    for (const group of series.groups) {
      if (groupTitles.has(group.title.trim()) || !Array.isArray(group.articles)) throw new Error('组标题不能重复，且需要 articles 数组。');
      groupTitles.add(group.title.trim());
    }
    for (const container of [series, ...series.groups]) {
      for (const key of container.articles || []) {
        articleKey(key);
        if (articles.has(key)) throw new Error('一篇文章只能属于一个系列或组。');
        articles.add(key);
      }
    }
  }
  return data;
}
function catalogPath(contentRoot) { return safePath(path.dirname(contentRoot), 'data', 'collections.json'); }
function readCatalog(contentRoot) {
  const file = catalogPath(contentRoot);
  if (!fs.existsSync(file)) return { version: 1, series: [] };
  if (fs.statSync(file).size > 2 * 1024 * 1024) throw new Error('系列目录文件过大。');
  try { return validateCatalog(JSON.parse(fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, ''))); }
  catch (error) { throw new Error(`请检查 collections.json：${error.message}`); }
}
function containers(data) { return data.series.flatMap(series => [series, ...series.groups]); }
function catalogRevision(data) { return createHash('sha256').update(JSON.stringify(data)).digest('hex'); }
function updateCatalog(contentRoot, change, expectedRevision) {
  const file = catalogPath(contentRoot);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const lock = safePath(path.dirname(file), 'collections.json.lock');
  let handle;
  try { handle = fs.openSync(lock, 'wx'); }
  catch (error) { if (error.code === 'EEXIST') throw new Error('另一个窗口正在修改系列目录，请稍后重试。若上次异常退出，请检查 data/collections.json.lock。'); throw error; }
  const temp = safePath(path.dirname(file), `collections-${randomUUID()}.tmp`);
  try {
    const data = readCatalog(contentRoot);
    if (expectedRevision !== undefined && catalogRevision(data) !== expectedRevision) {
      const error = new Error('系列目录已被其他窗口修改，请刷新后重试。');
      error.code = 'CONFLICT'; throw error;
    }
    const result = change(data);
    validateCatalog(data);
    fs.writeFileSync(temp, JSON.stringify(data, null, 2) + '\n', { flag: 'wx' });
    fs.renameSync(temp, file);
    return result;
  } finally {
    if (fs.existsSync(temp)) fs.unlinkSync(temp);
    fs.closeSync(handle); fs.unlinkSync(lock);
  }
}
function createSeries(contentRoot, title, description = '') {
  title = titleText(title);
  return updateCatalog(contentRoot, data => {
    const series = { id: `s-${randomUUID().slice(0, 8)}`, title, description, articles: [], groups: [] };
    data.series.push(series); return series;
  });
}
function createGroup(contentRoot, seriesId, title, description = '') {
  title = titleText(title);
  return updateCatalog(contentRoot, data => {
    const series = data.series.find(item => item.id === seriesId);
    if (!series) throw new Error('系列已不存在，请重新选择。');
    const group = { id: `g-${randomUUID().slice(0, 8)}`, title, description, articles: [] };
    series.groups.push(group); return { ...group, seriesId };
  });
}
function assignArticle(contentRoot, key, targetId, options = {}) {
  articleKey(key);
  if (!fs.statSync(safePath(contentRoot, key)).isFile()) throw new Error('文章文件不存在。');
  return updateCatalog(contentRoot, data => {
    const lists = containers(data);
    const target = targetId ? lists.find(item => item.id === targetId) : undefined;
    if (targetId && !target) throw new Error('系列或组已不存在，请重新选择。');
    if (options.beforeKey != null && (!target || !(target.articles || []).includes(options.beforeKey))) throw new Error('目标文章已移动，请刷新后重试。');
    // Choosing the existing destination preserves order; an explicit beforeKey
    // requests an insertion (null means append), including within the same list.
    if ((target?.articles || []).includes(key) && (options.beforeKey === undefined || options.beforeKey === key)) return;
    for (const item of lists) if (item.articles) item.articles = item.articles.filter(article => article !== key);
    if (target) {
      target.articles ||= [];
      const index = options.beforeKey == null ? target.articles.length : target.articles.indexOf(options.beforeKey);
      target.articles.splice(index, 0, key);
    }
  }, options.expectedRevision);
}
function moveArticle(contentRoot, key, delta) {
  return updateCatalog(contentRoot, data => {
    const group = containers(data).find(g => (g.articles || []).includes(key));
    if (!group) return;
    const from = group.articles.indexOf(key), to = from + Math.sign(delta);
    if (to >= 0 && to < group.articles.length) [group.articles[from], group.articles[to]] = [group.articles[to], group.articles[from]];
  });
}
function createArticle(contentRoot, title, targetId) {
  title = titleText(title);
  if (targetId && !containers(readCatalog(contentRoot)).some(item => item.id === targetId)) throw new Error('系列或组已不存在，请重新选择。');
  let slug = Array.from(title.replace(/[<>:"/\\|?*\x00-\x1f]/g, '-').replace(/^[ .-]+|[ .-]+$/g, '')).slice(0, 80).join('').replace(/[ .]+$/g, '');
  if (!slug) throw new Error('标题需要包含可用于目录名的文字或数字。');
  if (/^(con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])(?:\.|$)/i.test(slug)) slug = `文章-${slug}`;
  const folder = safePath(contentRoot, 'posts', slug);
  fs.mkdirSync(safePath(contentRoot, 'posts'), { recursive: true });
  try { fs.mkdirSync(folder); }
  catch (error) { if (error.code === 'EEXIST') throw new Error('同名文章目录已经存在，请换一个标题。'); throw error; }
  const key = `posts/${slug}/index.md`, file = path.join(folder, 'index.md');
  const date = new Date(), localDate = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
  const quoted = JSON.stringify(title).replace(/\+/g, '\\u002b');
  fs.writeFileSync(file, `+++\ntitle = ${quoted}\ndate = ${localDate}\ndescription = ""\ndraft = true\n\n[taxonomies]\ntags = []\n+++\n\n## 问题与背景\n\n在这里开始写作。\n\n## 我的思考\n\n`, { flag: 'wx' });
  fs.mkdirSync(path.join(folder, 'figures'));
  if (targetId) {
    try { assignArticle(contentRoot, key, targetId); }
    catch (error) { throw new Error(`草稿已保存到 ${file}，但加入系列或组失败：${error.message}。可稍后使用“移动文章到系列或组”。`); }
  }
  return { key, file };
}
function listArticles(contentRoot) {
  const posts = safePath(contentRoot, 'posts');
  if (!fs.existsSync(posts)) return [];
  const result = [];
  for (const dir of fs.readdirSync(posts, { withFileTypes: true })) {
    if (!dir.isDirectory() || dir.isSymbolicLink()) continue;
    const key = `posts/${dir.name}/index.md`, file = safePath(contentRoot, key);
    if (!fs.existsSync(file)) continue;
    const text = fs.readFileSync(file, 'utf8'), header = /^\uFEFF?\+\+\+\s*\r?\n([\s\S]*?)^\+\+\+\s*$/m.exec(text);
    let meta = {};
    try { if (header) meta = toml.parse(header[1]); } catch { /* Keep partially edited articles visible. */ }
    result.push({ key, file, title: meta.title || dir.name, draft: meta.draft === true });
  }
  return result;
}
module.exports = { inside, safePath, articleKey, titleText, validateCatalog, catalogPath, readCatalog, createSeries, createGroup, assignArticle, moveArticle, createArticle, listArticles, containers, catalogRevision };
