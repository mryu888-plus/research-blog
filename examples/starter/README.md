# My Research Workbench

A personal blog and writing setup created from the Research Blog template. It comes with live preview, Markdown math, Typst figures, linked notes, Zotero integration, and one style file, `ZEN.md`, that you can change.

You can hand this directory to a coding agent such as Claude Code or Codex: `AGENTS.md` tells it where things live, what to run after each kind of change, and what to ask you before doing. The rest of this file is for doing the same by hand.

Install Python 3.11+ and Node.js 18+. The setup script below downloads Zola 0.23.4 and Typst 0.15.1 into `.tools/` and verifies them; you can also install them yourself and put them on `PATH`. Python helpers use the standard library. VS Code is optional; Rust is only needed for the Rust CLI and Agent.

```sh
python scripts/setup_tools.py
npm --prefix extensions/research-blog ci --ignore-scripts
npm --prefix extensions/research-blog run build
python scripts/build_site.py
python scripts/check_site.py
python scripts/editor.py preview
```

The public build is in `site/public/`. Preview includes drafts and watches content, figures, and `ZEN.md`; use the local URL printed by the command.

To build the editor extensions from source:

```sh
npm --prefix extensions/research-blog ci --ignore-scripts
npm --prefix extensions/research-blog run package
npm --prefix extensions/research-notes-theme ci --ignore-scripts
npm --prefix extensions/research-notes-theme run package
```

Install the resulting VSIX files through VS Code's **Install from VSIX** menu. Open an article and run **Research Blog: 预览网站（含草稿）**. Use **Research Blog: 打开文献工作台** for Zotero; sign in to Zotero yourself for synchronization. Local write integration requires Zotero 10+. Annotations are edited in Zotero's native PDF reader.

Change your title, author, and public URL in `site/config.toml`. The personal CV integration is disabled with `extra.profile_enabled = false`; `site/content/about.md` is a regular Markdown page. No personal profile or document library has been copied.

ZEN is the included default style. Define your own colors, fonts, article width and line height, basic spacing, figure parameters, and AI guidance in the two JSON blocks in `ZEN.md`, then run `node scripts/sync_zen.cjs`. Website preview watches this file, and normal builds also synchronize it. See the [custom style guide](docs/style-guide.md) for a partial edit example and parameter details. Keep `schemaVersion` and the `graphite` / `paper` palette keys.

Generated website CSS, editor themes, Typst templates, and image guidance consume that same source. Do not edit generated consumers directly; component layout can be customized in CSS. Fonts are not downloaded automatically, and themes do not change VS Code editor fonts. Installed extensions need to be packaged and installed again to receive changes; the existing theme names and extension IDs remain stable.

Example notes live under `site/content/posts/`. Keep unfinished notes as `draft = true`. This excludes them from the website build; files committed to a public Git repository remain public. Keep private notes outside that repository. Only reference metadata used by published articles enters the public literature page; local PDF files, authorization keys, and private caches stay outside the website.

To publish with GitHub Pages, create an empty repository and set **Settings → Pages → Source** to **GitHub Actions**. Set `base_url` in `site/config.toml` to the actual Pages URL, such as `https://USERNAME.github.io/REPO/`, then push this workspace:

```sh
git init -b main
git add .
git commit -m "Start research notes"
git remote add origin https://github.com/USERNAME/REPO.git
git push -u origin main
```

The included `.github/workflows/pages.yml` builds and checks the website before publishing. Pull requests run the build and checks without deploying. Check the repository's Actions run for the deployment result and URL.

Shared code and these starter examples use the [MIT license](LICENSE). See [content licensing](CONTENT_LICENSE.md), [third-party notices](THIRD_PARTY_NOTICES.md), and the [getting started guide](docs/getting-started.md). You choose the license for your own writing and assets.

The full repository test suite and CI workflows belong to the upstream source checkout. Follow `CONTRIBUTING.md` there when contributing changes; this generated workspace contains the application and plugin sources for your own site.
