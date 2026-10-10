# Studio renders (dev only)

Renders the product pictures in `apps/web/public/media/renders/`. The WebP/PNG results are committed; this
tool is not part of the web build and CI never runs it.

```bash
bun apps/web/tools/render-studio/render.mjs            # all shots
bun apps/web/tools/render-studio/render.mjs hero-34    # selected shots by name
```

Needs the installed Google Chrome (`/Applications/Google Chrome.app`, or set `CHROME_PATH`). No browser is
downloaded. `three` and `puppeteer-core` are pinned devDependencies of `apps/web`.

How it works:

- `render.mjs` serves `scene.html` on 127.0.0.1 while it runs and drives Chrome with `puppeteer-core`.
- The enclosure STLs are read from `hardware/enclosure/export/` at render time only (Carrier is left out). They
  are never copied into `public/` or served by the site.
- `scene.js` puts every part back in its assembled place. The STLs are in print orientation, so each part gets
  the inverse of `print_pose` in `hardware/enclosure/build_case.py` plus its original bounding-box minimum
  (taken from the build script). If the enclosure model changes, re-derive those offsets (the numbers in
  `PARTS`), then re-render.
- The screen texture comes from `public/media/screens/` (see `../export-screens.mjs`).
- Output: 2400 px wide WebP (quality 90) and a `-1200` variant, plus a transparent `hero-34-transparent` PNG/WebP.
