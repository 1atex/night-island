// Night Island application icon: one description of the shapes, two outputs.
//   node gen.js   ->  icon.svg (source for docs/web), night-island.ico (16..256), icon-256.png
// No dependencies. The raster path is an own small anti-aliased renderer (signed distance
// shapes, supersampled) so the .ico does not depend on an SVG library. Run it after editing
// the shapes below and commit the results (CI does not run it).
//
// Concept "B": a dark rounded square holding a black capsule (the island) with a crescent moon
// and two status dots. Sizes up to 32 px use a simplified drawing (bigger capsule, no border,
// no stars) so the tray icon stays readable.
'use strict';
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const C = {
  bg: '#101533',
  capsule: '#05060f',
  border: '#2b3566',
  moon: '#f2e9c9',
  teal: '#5eead4',
  lavender: '#8b93c9',
};

// Everything is drawn on a 256 x 256 canvas. Layers are painted in order.
const FULL = {
  square: { x: 0, y: 0, w: 256, h: 256, r: 58 },
  capsule: { x: 24, y: 88, w: 208, h: 80, border: 2.5 },
  moon: { cx: 70, cy: 128, r: 26, cut: { dx: 11, dy: -6, r: 22 } },
  dots: [
    { cx: 140, cy: 128, r: 11, color: C.teal },
    { cx: 182, cy: 128, r: 11, color: C.lavender },
  ],
  stars: [
    { cx: 52, cy: 52, r: 4 },
    { cx: 112, cy: 38, r: 2.5 },
    { cx: 196, cy: 58, r: 3.5 },
    { cx: 214, cy: 206, r: 3 },
    { cx: 80, cy: 208, r: 2.5 },
    { cx: 150, cy: 214, r: 4 },
  ],
};
const SMALL = {
  square: { x: 0, y: 0, w: 256, h: 256, r: 58 },
  capsule: { x: 10, y: 74, w: 236, h: 108, border: 0 },
  moon: { cx: 66, cy: 128, r: 36, cut: { dx: 15, dy: -8, r: 30 } },
  dots: [
    { cx: 148, cy: 128, r: 16, color: C.teal },
    { cx: 206, cy: 128, r: 16, color: C.lavender },
  ],
  stars: [],
};

const hex = (s) => [1, 3, 5].map((i) => parseInt(s.slice(i, i + 2), 16));

// ---------- SVG ----------
function svg(d) {
  const cap = d.capsule;
  const m = d.moon;
  const out = [];
  out.push(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">`);
  out.push(`  <title>Night Island</title>`);
  out.push(`  <defs>`);
  out.push(`    <mask id="crescent" maskUnits="userSpaceOnUse" x="0" y="0" width="256" height="256">`);
  out.push(`      <rect width="256" height="256" fill="#fff"/>`);
  out.push(`      <circle cx="${m.cx + m.cut.dx}" cy="${m.cy + m.cut.dy}" r="${m.cut.r}" fill="#000"/>`);
  out.push(`    </mask>`);
  out.push(`  </defs>`);
  const q = d.square;
  out.push(`  <rect x="${q.x}" y="${q.y}" width="${q.w}" height="${q.h}" rx="${q.r}" fill="${C.bg}"/>`);
  for (const s of d.stars) out.push(`  <circle cx="${s.cx}" cy="${s.cy}" r="${s.r}" fill="${C.moon}" opacity="0.8"/>`);
  const b = cap.border;
  out.push(
    `  <rect x="${cap.x + b / 2}" y="${cap.y + b / 2}" width="${cap.w - b}" height="${cap.h - b}" rx="${(cap.h - b) / 2}" fill="${C.capsule}"` +
      (b ? ` stroke="${C.border}" stroke-width="${b}"` : '') +
      `/>`,
  );
  out.push(`  <circle cx="${m.cx}" cy="${m.cy}" r="${m.r}" fill="${C.moon}" mask="url(#crescent)"/>`);
  for (const t of d.dots) out.push(`  <circle cx="${t.cx}" cy="${t.cy}" r="${t.r}" fill="${t.color}"/>`);
  out.push(`</svg>`);
  return out.join('\n') + '\n';
}

// ---------- raster ----------
// Signed distance (negative inside) of the primitives.
const sdRoundRect = (px, py, x, y, w, h, r) => {
  const qx = Math.abs(px - (x + w / 2)) - (w / 2 - r);
  const qy = Math.abs(py - (y + h / 2)) - (h / 2 - r);
  return Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) + Math.min(Math.max(qx, qy), 0) - r;
};
const sdCircle = (px, py, cx, cy, r) => Math.hypot(px - cx, py - cy) - r;

// Colour of the drawing at a point (sample), as [r, g, b, a] with a in 0..1.
function sample(d, px, py) {
  const q = d.square;
  if (sdRoundRect(px, py, q.x, q.y, q.w, q.h, q.r) > 0) return [0, 0, 0, 0];
  let col = hex(C.bg);
  const paint = (c) => (col = c);
  for (const s of d.stars) if (sdCircle(px, py, s.cx, s.cy, s.r) <= 0) paint(hex(C.moon).map((v, i) => v * 0.8 + col[i] * 0.2));
  const cap = d.capsule;
  const sd = sdRoundRect(px, py, cap.x, cap.y, cap.w, cap.h, cap.h / 2);
  if (sd <= 0) paint(cap.border && sd > -cap.border ? hex(C.border) : hex(C.capsule));
  const m = d.moon;
  if (sdCircle(px, py, m.cx, m.cy, m.r) <= 0 && sdCircle(px, py, m.cx + m.cut.dx, m.cy + m.cut.dy, m.cut.r) > 0) paint(hex(C.moon));
  for (const t of d.dots) if (sdCircle(px, py, t.cx, t.cy, t.r) <= 0) paint(hex(t.color));
  return [col[0], col[1], col[2], 1];
}

function render(d, size) {
  const ss = size <= 48 ? 8 : 4;
  const px = Buffer.alloc(size * size * 4);
  const scale = 256 / size;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let r = 0, g = 0, b = 0, a = 0;
      for (let sy = 0; sy < ss; sy++) {
        for (let sx = 0; sx < ss; sx++) {
          const c = sample(d, (x + (sx + 0.5) / ss) * scale, (y + (sy + 0.5) / ss) * scale);
          r += c[0] * c[3]; g += c[1] * c[3]; b += c[2] * c[3]; a += c[3];
        }
      }
      const n = ss * ss;
      const o = (y * size + x) * 4;
      if (a > 0) {
        px[o] = Math.round(r / a); px[o + 1] = Math.round(g / a); px[o + 2] = Math.round(b / a);
        px[o + 3] = Math.round((a / n) * 255);
      }
    }
  }
  return px;
}

// ---------- PNG / ICO ----------
const crcTable = new Uint32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc32 = (buf) => {
  let c = 0xffffffff;
  for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}
function png(rgba, size) {
  const raw = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0;
    rgba.copy(raw, y * (size * 4 + 1) + 1, y * size * 4, (y + 1) * size * 4);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0); ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}
function ico(images) {
  const head = Buffer.alloc(6);
  head.writeUInt16LE(1, 2); head.writeUInt16LE(images.length, 4);
  const dir = Buffer.alloc(16 * images.length);
  let offset = 6 + dir.length;
  images.forEach(({ size, data }, i) => {
    const o = i * 16;
    dir[o] = size === 256 ? 0 : size; dir[o + 1] = size === 256 ? 0 : size;
    dir.writeUInt16LE(1, o + 4); dir.writeUInt16LE(32, o + 6);
    dir.writeUInt32LE(data.length, o + 8); dir.writeUInt32LE(offset, o + 12);
    offset += data.length;
  });
  return Buffer.concat([head, dir, ...images.map((i) => i.data)]);
}

const here = __dirname;
const sizes = [16, 24, 32, 48, 64, 128, 256];
const images = sizes.map((size) => ({ size, data: png(render(size <= 32 ? SMALL : FULL, size), size) }));
fs.writeFileSync(path.join(here, 'night-island.ico'), ico(images));
fs.writeFileSync(path.join(here, 'icon-256.png'), images[images.length - 1].data);
fs.writeFileSync(path.join(here, 'icon-small-64.png'), png(render(SMALL, 64), 64));
fs.writeFileSync(path.join(here, 'icon.svg'), svg(FULL));
fs.writeFileSync(path.join(here, 'icon-small.svg'), svg(SMALL));
console.log('wrote', sizes.join(', '));
