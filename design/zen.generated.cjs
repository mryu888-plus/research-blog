'use strict';
// Generated from ZEN.md by scripts/sync_zen.cjs. Do not edit.
module.exports = {
  "schemaVersion": 1,
  "name": "ZEN",
  "version": "1.0.0",
  "fonts": {
    "sans": [
      "Inter",
      "-apple-system",
      "BlinkMacSystemFont",
      "Segoe UI",
      "Noto Sans SC",
      "Microsoft YaHei",
      "sans-serif"
    ],
    "mono": [
      "Cascadia Code",
      "SFMono-Regular",
      "Consolas",
      "Liberation Mono",
      "monospace"
    ],
    "serif": [
      "Georgia",
      "Cambria",
      "Times New Roman",
      "serif"
    ],
    "figure": [
      "Arial",
      "Microsoft YaHei"
    ]
  },
  "layout": {
    "pageWidth": 1040,
    "readingWidth": 720,
    "bodyLineHeight": 1.8,
    "articleWidth": 700,
    "articleLineHeight": 1.95
  },
  "spacing": [
    8,
    12,
    16,
    24,
    32,
    48,
    64
  ],
  "radius": {
    "small": 3,
    "medium": 6,
    "large": 8
  },
  "motion": {
    "feedbackMs": 160
  },
  "figure": {
    "finePt": 0.6,
    "strongPt": 1,
    "labelPt": 10,
    "marginPt": 20
  },
  "print": {
    "background": "#ffffff",
    "surface": "#ffffff",
    "raised": "#eeeeee",
    "border": "#cccccc",
    "ink": "#000000",
    "secondary": "#333333",
    "muted": "#444444",
    "accent": "#000000"
  },
  "palettes": {
    "graphite": {
      "background": "#121213",
      "surface": "#171719",
      "raised": "#1e1e21",
      "border": "#2c2c30",
      "edge": "#424247",
      "ink": "#ededee",
      "secondary": "#aaaab2",
      "muted": "#92929b",
      "accent": "#de8b82",
      "accentMuted": "#babac3",
      "contrast": {
        "secondary": "#d6d6dc",
        "muted": "#bcbcc6",
        "border": "#666670"
      },
      "imageWords": "flat near-black graphite background, ivory-white fine ink lines, restrained cool gray, one muted terracotta-red accent",
      "effects": {
        "halo": "rgb(207 211 225 / .07)",
        "scanline": "rgb(229 230 238 / .055)",
        "nav": "rgb(18 18 19 / .88)",
        "edgeLight": "rgb(255 255 255 / .035)",
        "shadow": "0 16px 48px rgb(0 0 0 / .12)"
      },
      "editor": {
        "panel": "#171719",
        "raised": "#1e1e21",
        "string": "#b4bda5",
        "typeColor": "#adbccd",
        "number": "#c9b397",
        "function": "#ddd1be",
        "purple": "#beb0c8",
        "error": "#e99a93",
        "warning": "#d6bb8c",
        "success": "#a8be9b",
        "info": "#a7bdd0",
        "selection": "#2f292d",
        "inactiveSelection": "#2b272d",
        "find": "#342725",
        "line": "#1a1a1d",
        "buttonFg": "#121213",
        "buttonHover": "#e79c94",
        "shadow": "#00000050",
        "ansiBlack": "#6f6f79",
        "ansiWhite": "#d7d7dc",
        "ansiCyan": "#9dbfbd",
        "ansiBrightCyan": "#b1d0ce"
      }
    },
    "paper": {
      "background": "#f8f7f4",
      "surface": "#ffffff",
      "raised": "#efeeeb",
      "border": "#deddd8",
      "edge": "#bcbcc4",
      "ink": "#242427",
      "secondary": "#5e5e68",
      "muted": "#666670",
      "accent": "#9d4941",
      "accentMuted": "#5e5e68",
      "contrast": {
        "secondary": "#33333b",
        "muted": "#454550",
        "border": "#90909a"
      },
      "imageWords": "flat warm off-white background, charcoal-black fine ink lines, restrained gray, one dark terracotta-red accent",
      "effects": {
        "halo": "rgb(76 79 95 / .04)",
        "scanline": "rgb(56 56 70 / .035)",
        "nav": "rgb(248 247 244 / .88)",
        "edgeLight": "rgb(255 255 255 / .65)",
        "shadow": "0 16px 48px rgb(45 41 35 / .045)"
      },
      "editor": {
        "panel": "#efeeeb",
        "raised": "#ffffff",
        "string": "#4c6248",
        "typeColor": "#456078",
        "number": "#78572f",
        "function": "#65513a",
        "purple": "#745c80",
        "error": "#a63432",
        "warning": "#80591f",
        "success": "#48663d",
        "info": "#3f627f",
        "selection": "#eee5df",
        "inactiveSelection": "#eae6e1",
        "find": "#efe4da",
        "line": "#f0eeea",
        "buttonFg": "#ffffff",
        "buttonHover": "#883e37",
        "shadow": "#24242718",
        "ansiBlack": "#33333b",
        "ansiWhite": "#74747d",
        "ansiCyan": "#376967",
        "ansiBrightCyan": "#376967"
      }
    }
  },
  "prompts": {
    "image": {
      "opening": "Draw a clear, appealing explanatory illustration for a research blog using the ZEN design language. Meaning and readability come first.",
      "treatment": "a small amount of another muted color is welcome if it clarifies categories. Use confident clean contours, simple forms, subtle shading where helpful, and generous negative space.",
      "constraints": "Use only a few short, legible labels when needed. No article title, paragraphs, bullet lists, page furniture or four-panel lecture layout unless explicitly requested. Do not force engraved contour fields, abstract geometric decoration or a fixed diagram template. Preserve the source relationships and distinguish illustrative coordinates from real measurements. No invented results, pseudo-text or watermarks.",
      "compositions": {
        "editorial": "Create an engaging article illustration whose recognizable objects or schematic elements make the main idea intuitive. Give the composition room to breathe; choose the layout to suit the subject.",
        "cover": "Create an expressive topic illustration with a strong visual focus and quiet space for a separately typeset title. Choose a natural composition for the topic.",
        "concept": "Create an intuitive schematic showing the core relationship or contrast. Use recognizable subject drawings, vector cells, groups or arrows when they clarify the idea. Group comparisons naturally, not as a slide deck. Do not imply a sequential pipeline unless the source describes one."
      }
    },
    "typst": {
      "opening": "For Typst figure code, use the blog's ZEN restrained experimental-publication visual system when the surrounding code does not already define a style. Preserve existing palette variables, imports and layout.",
      "constraints": "Use named color variables, readable sans-serif labels and monospaced identifiers, generous spacing and aligned geometry. Use the configured accent sparingly for the key relation; no ornamental cards, gradients or decorative scientific marks. Exact counts, proportions, equations and arrow directions must follow the source. Render labels with Typst text, not raster assets. Prefer native Typst elements or the CeTZ version already imported; never invent an import or dependency version. Keep the cursor completion small: do not add page settings, imports or a full template in the middle of an expression. The source can use a blogdiagram starter with bg, ink, muted and accent variables; reuse them when present."
    }
  },
  "editorPalettes": {
    "graphite": {
      "name": "Research Notes Graphite",
      "type": "dark",
      "bg": "#121213",
      "fg": "#ededee",
      "secondary": "#aaaab2",
      "muted": "#92929b",
      "accent": "#de8b82",
      "border": "#2c2c30",
      "edge": "#424247",
      "panel": "#171719",
      "raised": "#1e1e21",
      "string": "#b4bda5",
      "typeColor": "#adbccd",
      "number": "#c9b397",
      "function": "#ddd1be",
      "purple": "#beb0c8",
      "error": "#e99a93",
      "warning": "#d6bb8c",
      "success": "#a8be9b",
      "info": "#a7bdd0",
      "selection": "#2f292d",
      "inactiveSelection": "#2b272d",
      "find": "#342725",
      "line": "#1a1a1d",
      "buttonFg": "#121213",
      "buttonHover": "#e79c94",
      "shadow": "#00000050",
      "ansiBlack": "#6f6f79",
      "ansiWhite": "#d7d7dc",
      "ansiCyan": "#9dbfbd",
      "ansiBrightCyan": "#b1d0ce"
    },
    "paper": {
      "name": "Research Notes Paper",
      "type": "light",
      "bg": "#f8f7f4",
      "fg": "#242427",
      "secondary": "#5e5e68",
      "muted": "#666670",
      "accent": "#9d4941",
      "border": "#deddd8",
      "edge": "#bcbcc4",
      "panel": "#efeeeb",
      "raised": "#ffffff",
      "string": "#4c6248",
      "typeColor": "#456078",
      "number": "#78572f",
      "function": "#65513a",
      "purple": "#745c80",
      "error": "#a63432",
      "warning": "#80591f",
      "success": "#48663d",
      "info": "#3f627f",
      "selection": "#eee5df",
      "inactiveSelection": "#eae6e1",
      "find": "#efe4da",
      "line": "#f0eeea",
      "buttonFg": "#ffffff",
      "buttonHover": "#883e37",
      "shadow": "#24242718",
      "ansiBlack": "#33333b",
      "ansiWhite": "#74747d",
      "ansiCyan": "#376967",
      "ansiBrightCyan": "#376967"
    }
  },
  "typstGuidance": "For Typst figure code, use the blog's ZEN restrained experimental-publication visual system when the surrounding code does not already define a style. Preserve existing palette variables, imports and layout. Use the configured light palette: background #f8f7f4, ink #242427, secondary #5e5e68, accent #9d4941; the dark palette uses background #121213, ink #ededee, secondary #aaaab2, accent #de8b82. Use 0.6pt fine strokes and 1pt primary strokes, 10-11pt labels. Use named color variables, readable sans-serif labels and monospaced identifiers, generous spacing and aligned geometry. Use the configured accent sparingly for the key relation; no ornamental cards, gradients or decorative scientific marks. Exact counts, proportions, equations and arrow directions must follow the source. Render labels with Typst text, not raster assets. Prefer native Typst elements or the CeTZ version already imported; never invent an import or dependency version. Keep the cursor completion small: do not add page settings, imports or a full template in the middle of an expression. The source can use a blogdiagram starter with bg, ink, muted and accent variables; reuse them when present."
};
