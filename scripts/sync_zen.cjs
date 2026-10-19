'use strict';
// ZEN.md is the authoring source. Generated consumers are safe to bundle/offline.
const fs = require('node:fs');
const path = require('node:path');
const ROOT = path.resolve(__dirname, '..');
const { renderThemes } = require('../extensions/research-notes-theme/scripts/build.cjs');

function block(markdown, name) {
  const expression = new RegExp(`<!-- zen:${name} -->\\s*\x60\x60\x60json\\s*([\\s\\S]*?)\\s*\x60\x60\x60\\s*<!-- /zen:${name} -->`, 'g');
  const matches = [...markdown.matchAll(expression)];
  if (matches.length !== 1) throw new Error(`ZEN.md must contain exactly one zen:${name} JSON block.`);
  return JSON.parse(matches[0][1]);
}

function readZen(markdown) {
  const tokens = block(markdown, 'tokens');
  const prompts = block(markdown, 'prompts');
  if (tokens.schemaVersion !== 1) throw new Error('Unsupported ZEN schema.');
  if (typeof tokens.name !== 'string' || !tokens.name.trim()) throw new Error('Invalid ZEN name.');
  const positive = (value, label) => {
    if (!Number.isFinite(value) || value <= 0) throw new Error(`Invalid ZEN ${label}.`);
  };
  for (const [group, keys] of Object.entries({
    layout: ['pageWidth', 'readingWidth', 'bodyLineHeight'], figure: ['finePt', 'strongPt', 'labelPt', 'marginPt'],
  })) {
    if (!tokens[group]) throw new Error(`Missing ZEN ${group}.`);
    for (const key of keys) positive(tokens[group][key], `${group}.${key}`);
  }
  for (const key of ['articleWidth', 'articleLineHeight']) {
    if (tokens.layout[key] !== undefined) positive(tokens.layout[key], `layout.${key}`);
  }
  for (const [group, keys] of Object.entries({ radius: ['small', 'medium', 'large'], motion: ['feedbackMs'] })) {
    for (const key of keys) {
      const value = tokens[group]?.[key];
      if (!Number.isFinite(value) || value < 0) throw new Error(`Invalid ZEN ${group}.${key}.`);
    }
  }
  if (!Array.isArray(tokens.spacing) || tokens.spacing.length !== 7) throw new Error('ZEN spacing must have seven steps.');
  tokens.spacing.forEach(value => positive(value, 'spacing'));
  for (const key of ['sans', 'mono', 'serif', 'figure']) {
    const fonts = tokens.fonts?.[key];
    if (!Array.isArray(fonts) || !fonts.length || fonts.some(font => typeof font !== 'string' || !font.trim())) throw new Error(`Invalid ZEN font ${key}.`);
  }
  const colors = ['background', 'surface', 'raised', 'border', 'edge', 'ink', 'secondary', 'muted', 'accent', 'accentMuted'];
  const editorColors = ['panel', 'raised', 'string', 'typeColor', 'number', 'function', 'purple', 'error', 'warning', 'success', 'info', 'selection', 'inactiveSelection', 'find', 'line', 'buttonFg', 'buttonHover', 'shadow', 'ansiBlack', 'ansiWhite', 'ansiCyan', 'ansiBrightCyan'];
  for (const key of ['background', 'surface', 'raised', 'border', 'ink', 'secondary', 'muted', 'accent']) {
    if (!/^#[\da-f]{6}$/i.test(tokens.print?.[key] || '')) throw new Error(`Invalid ZEN print color ${key}.`);
  }
  if (Object.keys(tokens.palettes || {}).sort().join(',') !== 'graphite,paper') throw new Error('ZEN requires Graphite and Paper palettes.');
  for (const name of ['graphite', 'paper']) {
    const palette = tokens.palettes?.[name];
    if (!palette) throw new Error(`Missing ZEN palette ${name}.`);
    for (const key of colors) {
      if (!/^#[\da-f]{6}$/i.test(palette[key] || '')) throw new Error(`Invalid ZEN color ${name}.${key}.`);
    }
    for (const key of editorColors) {
      if (!/^#[\da-f]{6}(?:[\da-f]{2})?$/i.test(palette.editor?.[key] || '')) throw new Error(`Invalid ZEN editor color ${name}.${key}.`);
    }
    for (const key of ['secondary', 'muted', 'border']) {
      if (!/^#[\da-f]{6}$/i.test(palette.contrast?.[key] || '')) throw new Error(`Invalid ZEN contrast color ${name}.${key}.`);
    }
    for (const key of ['halo', 'scanline', 'nav', 'edgeLight', 'shadow']) {
      if (typeof palette.effects?.[key] !== 'string' || !palette.effects[key].trim()) throw new Error(`Missing ZEN effect ${name}.${key}.`);
    }
    if (!palette.imageWords?.trim()) throw new Error(`Missing ZEN image description ${name}.`);
  }
  for (const text of [prompts.image?.opening, prompts.image?.treatment, prompts.image?.constraints,
    ...['editorial', 'cover', 'concept'].map(key => prompts.image?.compositions?.[key]),
    prompts.typst?.opening, prompts.typst?.constraints]) {
    if (typeof text !== 'string' || !text.trim()) throw new Error('Missing ZEN prompt rule.');
  }
  return { ...tokens, prompts };
}

const fontList = fonts => fonts.map(font => /^(?:sans-serif|serif|monospace|system-ui|-apple-system|BlinkMacSystemFont)$/.test(font) ? font : JSON.stringify(font)).join(', ');

function css(zen) {
  const paletteVars = p => ({
    'surface-0': p.background, 'surface-1': p.surface, 'surface-2': p.raised,
    'surface-3': p.border, 'surface-4': p.edge, 'text-primary': p.ink,
    'text-secondary': p.secondary, 'text-muted': p.muted, accent: p.accent, 'accent-muted': p.accentMuted,
    halo: p.effects.halo, scanline: p.effects.scanline, 'nav-background': p.effects.nav,
    'edge-light': p.effects.edgeLight, 'panel-shadow': p.effects.shadow,
  });
  const variables = values => Object.entries(values).map(([key, value]) => `  --${key}: ${value};`).join('\n');
  const common = {
    'font-sans': fontList(zen.fonts.sans), 'font-mono': fontList(zen.fonts.mono), 'font-serif': fontList(zen.fonts.serif),
    'radius-sm': `${zen.radius.small}px`, 'radius-md': `${zen.radius.medium}px`, 'radius-lg': `${zen.radius.large}px`, 'radius-full': '999px',
    'page-width': `${zen.layout.pageWidth}px`, 'reading-width': `${zen.layout.readingWidth}px`,
    'article-width': `${zen.layout.articleWidth ?? 700}px`, 'article-line-height': zen.layout.articleLineHeight ?? 1.95,
    'body-line-height': zen.layout.bodyLineHeight, 'feedback-duration': `${zen.motion.feedbackMs}ms`,
    ...Object.fromEntries(Object.entries(zen.palettes).flatMap(([key, p]) =>
      ['background', 'accent'].map(role => [`zen-${key}-${role}`, p[role]]))),
    // Keep slot names stable when authors change their spacing scale.
    ...Object.fromEntries([8, 12, 16, 24, 32, 48, 64].map((key, index) => [`space-${key}`, `${zen.spacing[index]}px`])),
    ...Object.fromEntries(['xs', 'sm', 'md', 'lg', 'xl', '2xl', '3xl'].map((key, index) => [`spacing-${key}`, `${zen.spacing[index]}px`])),
  };
  const contrast = p => variables({ 'text-secondary': p.contrast.secondary, 'text-muted': p.contrast.muted, 'surface-3': p.contrast.border });
  const p = zen.print;
  const print = variables({ 'surface-0': p.background, 'surface-1': p.surface, 'surface-2': p.raised, 'surface-3': p.border,
    'text-primary': p.ink, 'text-secondary': p.secondary, 'text-muted': p.muted, accent: p.accent });
  return `/* Generated from ZEN.md by scripts/sync_zen.cjs. Do not edit. */\n:root {\n  color-scheme: dark;\n${variables(common)}\n${variables(paletteVars(zen.palettes.graphite))}\n}\n:root[data-theme="light"] {\n  color-scheme: light;\n${variables(paletteVars(zen.palettes.paper))}\n}\n@media (prefers-contrast: more) {\n  :root {\n${contrast(zen.palettes.graphite)}\n  }\n  :root[data-theme="light"] {\n${contrast(zen.palettes.paper)}\n  }\n}\n@media print {\n  :root, :root[data-theme="light"] {\n${print}\n  }\n}\n`;
}

function editorPalettes(zen) {
  return Object.fromEntries(Object.entries(zen.palettes).map(([key, p]) => [key, {
    name: `Research Notes ${key === 'graphite' ? 'Graphite' : 'Paper'}`, type: key === 'graphite' ? 'dark' : 'light',
    bg: p.background, fg: p.ink, secondary: p.secondary, muted: p.muted, accent: p.accent,
    border: p.border, edge: p.edge, ...p.editor,
  }]));
}

function typstGuidance(zen) {
  const { paper: p, graphite: d } = zen.palettes;
  const f = zen.figure;
  return `${zen.prompts.typst.opening} Use the configured light palette: background ${p.background}, ink ${p.ink}, secondary ${p.secondary}, accent ${p.accent}; the dark palette uses background ${d.background}, ink ${d.ink}, secondary ${d.secondary}, accent ${d.accent}. Use ${f.finePt}pt fine strokes and ${f.strongPt}pt primary strokes, ${f.labelPt}-${f.labelPt + 1}pt labels. ${zen.prompts.typst.constraints}`;
}

function typstHeader(zen) {
  const { graphite: d, paper: p } = zen.palettes;
  return [
    '// zen:begin — generated from ZEN.md; keep this template standalone.',
    '#let dark = sys.inputs.at("theme", default: "paper") == "graphite"',
    ...Object.entries({ bg: 'background', ink: 'ink', muted: 'secondary', accent: 'accent' })
      .map(([key, role]) => `#let ${key} = if dark { rgb("${d[role]}") } else { rgb("${p[role]}") }`),
    `#let fine = ${zen.figure.finePt}pt`, `#let strong = ${zen.figure.strongPt}pt`,
    `#set page(width: auto, height: auto, margin: ${zen.figure.marginPt}pt, fill: bg)`,
    `#set text(font: (${zen.fonts.figure.map(font => JSON.stringify(font)).join(', ')}), size: ${zen.figure.labelPt}pt, fill: ink)`,
    '// zen:end',
  ].join('\n');
}

function renderOutputs(root, zen) {
  const read = name => fs.readFileSync(path.join(root, name), 'utf8').replace(/\r\n/g, '\n');
  const data = { ...zen, editorPalettes: editorPalettes(zen), typstGuidance: typstGuidance(zen) };
  const typstPath = 'design/visual-system/typst/blog-diagram.typ';
  const source = read(typstPath);
  const slot = /\/\/ zen:begin[^\n]*\n[\s\S]*?\/\/ zen:end/g;
  if ([...source.matchAll(slot)].length !== 1) throw new Error('Typst template must contain exactly one ZEN parameter slot.');
  const typst = source.replace(slot, () => typstHeader(zen));
  const snippetsPath = 'extensions/research-blog/snippets/blog.code-snippets';
  const snippets = JSON.parse(read(snippetsPath));
  const snippet = snippets['博客 Typst 图形模板'];
  if (snippet?.prefix !== 'blogdiagram') throw new Error('Missing blogdiagram snippet.');
  snippet.description = 'ZEN 图形：配色、线宽、标签与精确几何';
  // Literal dollars/backslashes must survive the VS Code snippet parser.
  snippet.body = typst.trimEnd().split('\n').map(line => line.replace(/\\/g, '\\\\').replace(/\$/g, '\\$'));
  const outputs = new Map([
    ['site/static/zen.css', css(zen)],
    ['design/zen.generated.cjs', `'use strict';\n// Generated from ZEN.md by scripts/sync_zen.cjs. Do not edit.\nmodule.exports = ${JSON.stringify(data, null, 2)};\n`],
    [typstPath, typst],
    [snippetsPath, JSON.stringify(snippets, null, 2) + '\n'],
  ]);
  const themeTemplate = read('extensions/research-notes-theme/scripts/preview.template.html');
  for (const [name, content] of renderThemes(data.editorPalettes, themeTemplate)) {
    outputs.set(`extensions/research-notes-theme/${name}`, content);
  }
  return outputs;
}

function sync({ root = ROOT, check = false } = {}) {
  const zen = readZen(fs.readFileSync(path.join(root, 'ZEN.md'), 'utf8'));
  // Render and validate everything before writing any consumer.
  const outputs = renderOutputs(root, zen);
  const changed = [];
  for (const [relative, content] of outputs) {
    const filename = path.join(root, relative);
    if (fs.existsSync(filename) && fs.readFileSync(filename, 'utf8').replace(/\r\n/g, '\n') === content) continue;
    changed.push(relative);
    if (!check) {
      fs.mkdirSync(path.dirname(filename), { recursive: true });
      fs.writeFileSync(filename, content);
    }
  }
  if (check && changed.length) throw new Error(`ZEN generated files are stale. Run node scripts/sync_zen.cjs:\n${changed.join('\n')}`);
  return changed;
}

module.exports = { readZen, renderOutputs, sync };
if (require.main === module) {
  try {
    const changed = sync({ check: process.argv.includes('--check') });
    console.log(`ZEN: ${changed.length ? `synced ${changed.length} files` : 'all consumers are current'}.`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
