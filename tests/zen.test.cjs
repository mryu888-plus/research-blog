'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');
const { readZen, renderOutputs, sync } = require('../scripts/sync_zen.cjs');
const ROOT = path.resolve(__dirname, '..');
const source = fs.readFileSync(path.join(ROOT, 'ZEN.md'), 'utf8');
const zen = readZen(source);

function markdownWithStyle(style) {
  const { prompts, ...tokens } = style;
  return source
    .replace(/(<!-- zen:tokens -->\s*```json\s*)[\s\S]*?(\s*```\s*<!-- \/zen:tokens -->)/,
      (_, opening, closing) => opening + JSON.stringify(tokens, null, 2) + closing)
    .replace(/(<!-- zen:prompts -->\s*```json\s*)[\s\S]*?(\s*```\s*<!-- \/zen:prompts -->)/,
      (_, opening, closing) => opening + JSON.stringify(prompts, null, 2) + closing);
}

function generatedData(code) {
  const context = { module: { exports: {} } };
  vm.runInNewContext(code, context);
  return context.module.exports;
}

function adapterWithStyle(relative, data) {
  const context = {
    module: { exports: {} },
    require(filename) {
      assert.equal(filename, '../../../design/zen.generated.cjs');
      return data;
    },
  };
  vm.runInNewContext(fs.readFileSync(path.join(ROOT, relative), 'utf8'), context);
  return context.module.exports;
}

test('a user-defined style synchronizes colors, fonts, dimensions and AI rules without fixed color names', t => {
  const changed = structuredClone(zen);
  changed.name = '青蓝实验室';
  changed.fonts = {
    sans: ['Source Sans 3', 'sans-serif'], mono: ['JetBrains Mono', 'monospace'],
    serif: ['Source Serif 4', 'serif'], figure: ['Noto Sans', 'Arial'],
  };
  changed.layout = { pageWidth: 980, readingWidth: 680, bodyLineHeight: 1.6, articleWidth: 620, articleLineHeight: 1.7 };
  changed.spacing = [4, 9, 14, 20, 28, 40, 56];
  changed.radius = { small: 0, medium: 0, large: 0 };
  changed.motion.feedbackMs = 0;
  changed.figure = { finePt: 0.8, strongPt: 1.3, labelPt: 11, marginPt: 24 };
  Object.assign(changed.palettes.paper, {
    background: '#f2f9fc', ink: '#17334a', secondary: '#476577', accent: '#007d80',
    imageWords: 'cool white background, navy ink, slate secondary, one teal accent',
  });
  Object.assign(changed.palettes.graphite, {
    background: '#0a1d30', ink: '#dfedf8', secondary: '#89a9bd', accent: '#60d5d0',
    imageWords: 'deep navy background, crisp pale blue ink, muted slate, one teal accent',
  });
  changed.prompts.image = {
    opening: "Use my lab's visual language.", treatment: 'Prefer clear geometry and a clean background.',
    constraints: 'Show exact relations with a restrained accent; retain source fidelity.',
    compositions: { editorial: 'Show the research subject.', cover: 'Leave room for a title.', concept: 'Show a direct comparison.' },
  };
  changed.prompts.typst = {
    opening: 'Create a precise laboratory figure.',
    constraints: 'Use the configured palette. Preserve arrow directions and source geometry.',
  };

  const fixture = fs.mkdtempSync(path.join(os.tmpdir(), 'zen-custom-'));
  t.after(() => fs.rmSync(fixture, { recursive: true, force: true }));
  for (const relative of ['design/visual-system/typst/blog-diagram.typ', 'extensions/research-blog/snippets/blog.code-snippets',
    'extensions/research-notes-theme/scripts/preview.template.html']) {
    const destination = path.join(fixture, relative);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.copyFileSync(path.join(ROOT, relative), destination);
  }
  fs.writeFileSync(path.join(fixture, 'ZEN.md'), markdownWithStyle(changed));
  assert(sync({ root: fixture }).length > 0);
  assert.deepEqual(sync({ root: fixture, check: true }), []);
  const read = relative => fs.readFileSync(path.join(fixture, relative), 'utf8');
  const css = read('site/static/zen.css');
  for (const [key, value] of Object.entries({
    'surface-0': '#f2f9fc', accent: '#007d80', 'page-width': '980px', 'reading-width': '680px',
    'body-line-height': '1.6', 'article-width': '620px', 'article-line-height': '1.7',
    'radius-sm': '0px', 'radius-md': '0px', 'radius-lg': '0px', 'feedback-duration': '0ms',
  })) assert(css.includes(`--${key}: ${value};`), key);
  for (const [index, key] of [8, 12, 16, 24, 32, 48, 64].entries()) {
    assert(css.includes(`--space-${key}: ${changed.spacing[index]}px;`));
  }
  for (const [index, key] of ['xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl'].entries()) {
    assert(css.includes(`--spacing-${key}: ${changed.spacing[index]}px;`));
  }
  assert(css.includes('--font-sans: "Source Sans 3", sans-serif;'));
  assert(css.includes('--font-mono: "JetBrains Mono", monospace;'));
  assert(css.includes('--font-serif: "Source Serif 4", serif;'));
  for (const appearance of ['graphite', 'paper']) {
    const theme = JSON.parse(read(`extensions/research-notes-theme/themes/research-notes-${appearance}-color-theme.json`));
    assert.equal(theme.colors['editor.background'], changed.palettes[appearance].background);
    assert.equal(theme.colors['editorCursor.foreground'], changed.palettes[appearance].accent);
  }
  const typst = read('design/visual-system/typst/blog-diagram.typ');
  assert(typst.includes('#let fine = 0.8pt'));
  assert(typst.includes('#let strong = 1.3pt'));
  assert(typst.includes('margin: 24pt'));
  assert(typst.includes('font: ("Noto Sans", "Arial"), size: 11pt'));
  for (const appearance of ['graphite', 'paper']) {
    for (const role of ['background', 'ink', 'secondary', 'accent']) assert(typst.includes(changed.palettes[appearance][role]));
  }
  const snippets = JSON.parse(read('extensions/research-blog/snippets/blog.code-snippets'));
  assert(snippets['博客 Typst 图形模板'].body.includes('#let fine = 0.8pt'));
  const data = generatedData(read('design/zen.generated.cjs'));
  assert.equal(data.name, changed.name);
  assert.deepEqual(Array.from(data.fonts.figure), changed.fonts.figure);
  const { artDirection } = adapterWithStyle('extensions/research-blog/src/image-style.cjs', data);
  const { TYPST_FIGURE_STYLE } = adapterWithStyle('extensions/research-blog/src/typst-style.cjs', data);
  assert(TYPST_FIGURE_STYLE.includes(changed.prompts.typst.opening));
  assert(TYPST_FIGURE_STYLE.includes(changed.prompts.typst.constraints));
  assert(TYPST_FIGURE_STYLE.includes('0.8pt') && TYPST_FIGURE_STYLE.includes('1.3pt'));
  assert(!/warm paper|charcoal|terracotta|ivory/i.test(TYPST_FIGURE_STYLE));
  for (const appearance of ['graphite', 'paper']) {
    const direction = artDirection(appearance, 'concept', 'Keep the source comparison.');
    assert(direction.includes(changed.palettes[appearance].imageWords));
    assert(direction.includes(changed.prompts.image.opening));
    assert(direction.includes(changed.prompts.image.constraints));
    assert(direction.includes(changed.prompts.image.compositions.concept));
    assert(direction.endsWith('Keep the source comparison.'));
    for (const role of ['background', 'ink', 'secondary', 'accent']) {
      assert(direction.includes(changed.palettes[appearance][role]));
      assert(TYPST_FIGURE_STYLE.includes(changed.palettes[appearance][role]));
    }
    assert(!/warm paper|charcoal|terracotta|ivory/i.test(direction));
  }
});

test('older style files retain the current article width and line height when optional fields are absent', () => {
  const legacy = structuredClone(zen);
  delete legacy.layout.articleWidth;
  delete legacy.layout.articleLineHeight;
  const css = renderOutputs(ROOT, readZen(markdownWithStyle(legacy))).get('site/static/zen.css');
  assert(css.includes('--article-width: 700px;'));
  assert(css.includes('--article-line-height: 1.95;'));
});

test('editor colors with alpha produce valid theme colors and derived overlays replace the input alpha', () => {
  const changed = structuredClone(zen);
  for (const palette of Object.values(changed.palettes)) {
    for (const key of Object.keys(palette.editor)) palette.editor[key] = palette.editor[key].slice(0, 7) + '80';
  }
  const outputs = renderOutputs(ROOT, readZen(markdownWithStyle(changed)));
  for (const appearance of ['graphite', 'paper']) {
    const theme = JSON.parse(outputs.get(`extensions/research-notes-theme/themes/research-notes-${appearance}-color-theme.json`));
    const editor = changed.palettes[appearance].editor;
    for (const [role, color] of Object.entries(theme.colors)) {
      assert.match(color, /^#[\da-f]{6}(?:[\da-f]{2})?$/i, `${appearance}: ${role}`);
    }
    assert.equal(theme.colors['selection.background'], editor.selection);
    assert.equal(theme.colors['list.dropBackground'], editor.selection.slice(0, 7) + 'aa');
    assert.equal(theme.colors['editor.wordHighlightBackground'], editor.typeColor.slice(0, 7) + '16');
    assert.equal(theme.colors['editorOverviewRuler.addedForeground'], editor.success.slice(0, 7) + 'aa');
    assert.equal(theme.colors['diffEditor.insertedTextBackground'], editor.success.slice(0, 7) + '24');
    assert.equal(theme.colors['diffEditor.insertedLineBackground'], editor.success.slice(0, 7) + '0d');
    assert.equal(theme.colors['diffEditor.removedTextBackground'], editor.error.slice(0, 7) + '24');
    // Source-control foregrounds keep the author-specified alpha; only overlays replace it.
    assert.equal(theme.colors['gitDecoration.addedResourceForeground'], editor.success);
    assert.equal(theme.colors['gitDecoration.modifiedResourceForeground'], editor.info);
    assert.equal(theme.colors['gitDecoration.conflictingResourceForeground'], editor.warning);
  }
});

test('invalid custom names and dimensions fail before any generated file is written', t => {
  for (const name of ['', '   ', null, 42]) {
    const changed = structuredClone(zen);
    changed.name = name;
    assert.throws(() => readZen(markdownWithStyle(changed)), /name/i);
  }
  for (const [group, keys] of Object.entries({ layout: ['pageWidth', 'readingWidth', 'bodyLineHeight', 'articleWidth', 'articleLineHeight'],
    radius: ['small', 'medium', 'large'], motion: ['feedbackMs'], figure: ['finePt', 'strongPt', 'labelPt', 'marginPt'] })) {
    for (const key of keys) {
      const changed = structuredClone(zen);
      changed[group][key] = -1;
      assert.throws(() => readZen(markdownWithStyle(changed)), new RegExp(`${group}\\.${key}`));
    }
  }
  for (const key of ['articleWidth', 'articleLineHeight']) {
    const changed = structuredClone(zen);
    changed.layout[key] = 0;
    assert.throws(() => readZen(markdownWithStyle(changed)), new RegExp(`layout\\.${key}`));
  }
  const fixture = fs.mkdtempSync(path.join(os.tmpdir(), 'zen-invalid-'));
  t.after(() => fs.rmSync(fixture, { recursive: true, force: true }));
  const changed = structuredClone(zen);
  changed.layout.articleWidth = -1;
  fs.writeFileSync(path.join(fixture, 'ZEN.md'), markdownWithStyle(changed));
  fs.mkdirSync(path.join(fixture, 'site/static'), { recursive: true });
  const output = path.join(fixture, 'site/static/zen.css');
  fs.writeFileSync(output, 'existing generated consumer');
  assert.throws(() => sync({ root: fixture }), /layout\.articleWidth/);
  assert.equal(fs.readFileSync(output, 'utf8'), 'existing generated consumer');
  assert(!fs.existsSync(path.join(fixture, 'design/zen.generated.cjs')));
});

test('a change in ZEN reaches the website, editor, figure and AI consumers', () => {
  const changed = structuredClone(zen);
  changed.palettes.paper.background = '#faf9f0';
  changed.palettes.paper.accent = '#913f38';
  changed.figure.finePt = 0.7;
  changed.motion.feedbackMs = 175;
  const outputs = renderOutputs(ROOT, changed);
  const css = outputs.get('site/static/zen.css');
  assert(css.includes('--surface-0: #faf9f0;'));
  assert(css.includes('--accent: #913f38;'));
  assert(css.includes('--feedback-duration: 175ms;'));
  const theme = JSON.parse(outputs.get('extensions/research-notes-theme/themes/research-notes-paper-color-theme.json'));
  assert.equal(theme.colors['editor.background'], '#faf9f0');
  assert.equal(theme.colors['editorCursor.foreground'], '#913f38');
  // Diagnostics and categorical syntax retain their separate semantics.
  assert.equal(theme.colors['editorError.foreground'], changed.palettes.paper.editor.error);
  assert.equal(theme.semanticTokenColors.string, changed.palettes.paper.editor.string);
  const typst = outputs.get('design/visual-system/typst/blog-diagram.typ');
  assert(typst.includes('rgb("#faf9f0")') && typst.includes('#let fine = 0.7pt'));
  const snippet = JSON.parse(outputs.get('extensions/research-blog/snippets/blog.code-snippets'));
  assert(snippet['博客 Typst 图形模板'].body.includes('#let fine = 0.7pt'));
  const context = { module: { exports: {} } };
  vm.runInNewContext(outputs.get('design/zen.generated.cjs'), context);
  assert.equal(context.module.exports.palettes.paper.accent, '#913f38');
  assert(context.module.exports.typstGuidance.includes('0.7pt'));
  assert(context.module.exports.typstGuidance.includes('#faf9f0'));
});

test('malformed or incomplete ZEN data fails before writing consumers', () => {
  assert.throws(() => readZen(source.replace('<!-- zen:tokens -->', '<!-- missing -->')), /exactly one/);
  assert.throws(() => readZen(source + '\n' + source), /exactly one/);
  assert.throws(() => readZen(source.replace('"schemaVersion": 1', '"schemaVersion": 99')), /schema/);
  assert.throws(() => readZen(source.replace('"pageWidth": 1040', '"pageWidth": -1')), /layout.pageWidth/);
  assert.throws(() => readZen(source.replace('"feedbackMs": 160', '"missing": 160')), /feedbackMs/);
  assert.throws(() => readZen(source.replace('"#121213"', '"not a color"')), /color/);
});

test('all generated consumers are synchronized and checks do not write files', () => {
  assert.deepEqual(sync({ check: true }), []);
  const outputs = renderOutputs(ROOT, zen);
  for (const [relative, expected] of outputs) {
    assert.equal(fs.readFileSync(path.join(ROOT, relative), 'utf8').replace(/\r\n/g, '\n'), expected, relative);
  }
});

test('AI adapters use the shared rules and retain flexible composition and source fidelity', () => {
  const { artDirection } = require('../extensions/research-blog/src/image-style.cjs');
  const { TYPST_FIGURE_STYLE } = require('../extensions/research-blog/src/typst-style.cjs');
  for (const appearance of ['graphite', 'paper']) {
    const direction = artDirection(appearance, 'concept', 'Show the source contrast.');
    for (const role of ['background', 'ink', 'secondary', 'accent']) assert(direction.includes(zen.palettes[appearance][role]));
    assert(direction.includes(zen.prompts.image.constraints));
    assert(direction.includes('Do not imply a sequential pipeline'));
    assert(direction.endsWith('Show the source contrast.'));
  }
  assert(TYPST_FIGURE_STYLE.includes('arrow directions must follow the source'));
  assert(TYPST_FIGURE_STYLE.includes('Preserve existing palette variables'));
  assert.throws(() => artDirection('toString', 'concept'), /未知/);
  assert.throws(() => artDirection('paper', '__proto__'), /未知/);
});

test('every blog page loads ZEN before layouts and old styles cannot override its palettes', () => {
  const base = fs.readFileSync(path.join(ROOT, 'site/templates/base.html'), 'utf8');
  assert(base.indexOf("path='zen.css'") < base.indexOf("path='style.css'"));
  for (const filename of ['style.css', 'apollo.css', 'graph.css', 'topics.css', 'series.css', 'about.css']) {
    const css = fs.readFileSync(path.join(ROOT, 'site/static', filename), 'utf8');
    assert(!/--(?:surface-[0-4]|text-primary|text-secondary|text-muted|accent):\s*#/.test(css), filename);
  }
});
