// Night Island application icon: renders the SVG sources into night-island.ico and icon-256.png.
//   node gen.js      (needs ImageMagick 7 `magick` with the RSVG delegate in PATH)
// Every size is rendered from icon.svg. icon-alt-island.svg is the spare variant, not built.
// Run it after editing the SVGs and commit the results (CI does not run it).
'use strict';
const { execFileSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const dir = __dirname;
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'night-island-icon-'));
const SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256];

function render(svg, size) {
  const out = path.join(tmp, `${path.basename(svg, '.svg')}-${size}.png`);
  execFileSync('magick', [
    '-background', 'none', '-density', String(Math.ceil((96 * size) / 256) * 4),
    path.join(dir, svg), '-resize', `${size}x${size}`, `PNG32:${out}`,
  ]);
  return out;
}

const frames = SIZES.map((s) => render('icon.svg', s));
execFileSync('magick', [...frames, path.join(dir, 'night-island.ico')]);
fs.copyFileSync(frames[frames.length - 1], path.join(dir, 'icon-256.png'));
fs.rmSync(tmp, { recursive: true, force: true });
console.log('wrote night-island.ico (16 20 24 32 40 48 64 128 256) and icon-256.png');
