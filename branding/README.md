# Branding

Everything here is made by `generate.mjs`: the logo from one drawing, the logo's source,
`dashboard/src/assets/atm-logo.svg`, drawn in `currentColor` (the dashboard's logo, favicon and About come straight
from it); the social preview from that and the dashboard itself. Never edit these files by hand: change the source,
the demo project or the pages in `generate.mjs`, then regenerate.

| File | What it is | Where it's used |
|---|---|---|
| `atm-logo.svg` | The logo in the accent blue (`#2f81f7`, the favicon's colour) | The README's header |
| `atm-logo.png` | 512×512, the blue logo on the dark background (`#0d1117`) | The GitHub org's avatar (avatars take only PNG, JPG or GIF) |
| `social-preview-dark.png`, `social-preview-light.png` | 1280×640: the logo, name and tagline, how to install, and the dashboard (T-279), on dark and on light | GitHub's social preview, shown when the repository is linked: one of them |
| `demo-tasks.json` | The demo project the social preview's dashboard shows: a small shop's sprint, two agents at work, a review, a question | `generate.mjs` |
| `generate.mjs` | The script that makes them, with the HTML it renders the PNGs from | `pnpm run branding` |

## The social preview's inputs

- **The dashboard:** the built one, `plugin/templates/tasks-dashboard.html` (run `pnpm run build` first), served the
  demo project from a page route the way the e2e tests serve their fixtures, on the Kanban board, at a fixed time
  (`DEMO_NOW`), with the demo's two agents as live claims; in the dark theme for the dark card and in light for the
  light one. So the card shows the real product, and follows its changes when regenerated.
- **The words:** the tagline (`TAGLINE`) and the two install commands (`INSTALL`), as the README gives them.
- **The layout:** `PREVIEW_LAYOUT` ("a": the words on the left, the dashboard on the right). `--variants <dir>` renders
  every layout, dark and light, into `<dir>` to compare.

## Regenerate

In `dashboard/` (where Playwright is installed), after `pnpm run build`:

```bash
pnpm run branding
pnpm run branding --variants /tmp/previews   # every layout, to choose from
```

It rewrites the files above. Then upload what changed by hand, GitHub doesn't read them from the repository:
- `social-preview-dark.png` or `social-preview-light.png`: the repository's Settings → General → Social preview;
- `atm-logo.png`: the org's Settings → Profile → Profile picture.

`dashboard/tests/unit/readme-logo.test.ts` fails when `atm-logo.svg` no longer matches the source. The PNGs aren't
compared (they differ by platform): look at them before committing.
