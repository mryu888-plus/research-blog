"use strict";
const PALETTES = {
  graphite: { background: "#121213", ink: "#ededee", secondary: "#aaaab2", accent: "#de8b82", words: "flat near-black graphite background, ivory-white fine ink lines, restrained cool gray, one muted terracotta-red accent" },
  paper: { background: "#f8f7f4", ink: "#242427", secondary: "#5e5e68", accent: "#9d4941", words: "flat warm off-white background, charcoal-black fine ink lines, restrained gray, one dark terracotta-red accent" }
};
const COMPOSITIONS = {
  editorial: "One large abstract visual metaphor occupying about 60% of the canvas, slightly off-center; balance a dense region of precision linework against a quiet open region. The subject must read at thumbnail size. Favor a sculptural contour field, layered cross-section, folded plane, or geometric interference pattern chosen to express the actual topic.",
  cover: "A wide art plate with one monumental abstract form occupying the right two-thirds; the entire left third must remain blank background with absolutely no marks or text. Use a strong silhouette and a single visual tension; avoid a decorative collage.",
  concept: "A legible conceptual plate with at most three principal forms arranged on a clear common axis. Express the actual relationship using adjacency, containment, alignment or a shared contour. Use no decorative curves if they misrepresent the concept. This is an explanatory metaphor, not a measured scientific chart."
};
function artDirection(appearance = "graphite", composition = "editorial", extra = "") {
  const palette = PALETTES[appearance];
  if (!palette || !COMPOSITIONS[composition]) throw new Error("未知的配图风格，请选择石墨黑或暖纸白，以及有效的配图用途。");
  return `Create a refined abstract editorial illustration with no typography whatsoever.
Use ${palette.words}. The red accent occupies at most 3% of the image and identifies a single focal point. The background is absolutely uniform.
Use precise engraved linework, subtle contour rhythm, controlled line density, sharp geometric edges, one coherent object family. Two line weights, stronger structural contours and fine internal lines, clearly visible at web size. Flat ink; depth comes from line spacing and occlusion, never glossy lighting.
${COMPOSITIONS[composition]}
Keep an 8% outer safe margin and 25-35% quiet negative space, with intentional visual balance.
Exclude stock document icons, magnifying glasses, funnel icons, clipart, bubbles connected by generic arrows, node-soup, dashboard panels, rounded cards, 3D product rendering, glow, gradients, drop shadows, busy texture, generated typography, watermarks, logos, arbitrary scientific data or equations. Do not draw a physical book or page.
No letters, numbers, captions, decorative type or pseudo-text anywhere in the image.
${extra.trim() ? extra.trim() : ""}`.trim();
}
module.exports = { PALETTES, COMPOSITIONS, artDirection };
