'use strict';
// esbuild embeds this data in the VSIX; no workspace file is read at runtime.
const zen = require('../../../design/zen.generated.cjs');
const PALETTES = Object.fromEntries(Object.entries(zen.palettes).map(([key, p]) => [key, {
  background: p.background, ink: p.ink, secondary: p.secondary, accent: p.accent, words: p.imageWords,
}]));
const COMPOSITIONS = zen.prompts.image.compositions;
function artDirection(appearance = 'paper', composition = 'concept', extra = '') {
  const palette = PALETTES[appearance];
  if (!Object.hasOwn(PALETTES, appearance) || !Object.hasOwn(COMPOSITIONS, composition)) throw new Error('未知的配图风格，请选择深色或浅色风格，以及有效的配图用途。');
  return `${zen.prompts.image.opening}\nUse ${palette.words} (background ${palette.background}, ink ${palette.ink}, secondary ${palette.secondary}, accent ${palette.accent}); ${zen.prompts.image.treatment}\n${COMPOSITIONS[composition]}\n${zen.prompts.image.constraints}\n${extra.trim() ? extra.trim() : ''}`.trim();
}
module.exports = { PALETTES, COMPOSITIONS, artDirection };
