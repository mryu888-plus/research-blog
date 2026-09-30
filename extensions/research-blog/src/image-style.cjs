'use strict';
const PALETTES = {
  graphite: { background: '#121213', ink: '#ededee', secondary: '#aaaab2', accent: '#de8b82', words: 'flat near-black graphite background, ivory-white fine ink lines, restrained cool gray, one muted terracotta-red accent' },
  paper: { background: '#f8f7f4', ink: '#242427', secondary: '#5e5e68', accent: '#9d4941', words: 'flat warm off-white background, charcoal-black fine ink lines, restrained gray, one dark terracotta-red accent' },
};
const COMPOSITIONS = {
  editorial: 'Create an engaging article illustration whose recognizable objects or schematic elements make the main idea intuitive. Give the composition room to breathe; choose the layout to suit the subject.',
  cover: 'Create an expressive topic illustration with a strong visual focus and quiet space for a separately typeset title. Choose a natural composition for the topic.',
  concept: 'Create an intuitive schematic showing the core relationship or contrast. Use recognizable subject drawings, vector cells, groups or arrows when they clarify the idea. Group comparisons naturally, not as a slide deck. Do not imply a sequential pipeline unless the source describes one.',
};
function artDirection(appearance = 'paper', composition = 'concept', extra = '') {
  const palette = PALETTES[appearance];
  if (!palette || !COMPOSITIONS[composition]) throw new Error('未知的配图风格，请选择石墨黑或暖纸白，以及有效的配图用途。');
  return `Draw a clear, appealing explanatory illustration for a research blog. Meaning and readability come first.\nUse ${palette.words}; a small amount of another muted color is welcome if it clarifies categories. Use confident clean contours, simple forms, subtle shading where helpful, and generous negative space.\n${COMPOSITIONS[composition]}\nUse only a few short, legible labels when needed. No article title, paragraphs, bullet lists, page furniture or four-panel lecture layout unless explicitly requested. Do not force engraved contour fields, abstract geometric decoration or a fixed diagram template. Preserve the source relationships and distinguish illustrative coordinates from real measurements. No invented results, pseudo-text or watermarks.\n${extra.trim() ? extra.trim() : ''}`.trim();
}
module.exports = { PALETTES, COMPOSITIONS, artDirection };
