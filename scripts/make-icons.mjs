// Generates the app icons (PNG + ICO) without any external dependency.
// Run with: npm run icons
import zlib from "node:zlib";
import fs from "node:fs";
import path from "node:path";

const OUT = path.join(process.cwd(), "src-tauri", "icons");
fs.mkdirSync(OUT, { recursive: true });

/* ---------- PNG encoder ---------- */
const CRC_TABLE = (() => {
  const table = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c;
  }
  return table;
})();

function crc32(buffer) {
  let c = 0xffffffff;
  for (const byte of buffer) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const typeBuffer = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])));
  return Buffer.concat([length, typeBuffer, data, crc]);
}

function encodePng(width, height, rgba) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8; // bit depth
  header[9] = 6; // RGBA
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (width * 4 + 1)] = 0;
    rgba.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", zlib.deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

/* ---------- drawing helpers (unit coordinates) ---------- */
const clamp01 = (v) => Math.min(1, Math.max(0, v));

function roundedBox(x, y, cx, cy, halfW, halfH, radius) {
  const dx = Math.abs(x - cx) - (halfW - radius);
  const dy = Math.abs(y - cy) - (halfH - radius);
  const outside = Math.hypot(Math.max(dx, 0), Math.max(dy, 0));
  const inside = Math.min(Math.max(dx, dy), 0);
  return outside + inside - radius;
}

function distanceToSegment(px, py, ax, ay, bx, by) {
  const vx = bx - ax;
  const vy = by - ay;
  const wx = px - ax;
  const wy = py - ay;
  const t = clamp01((wx * vx + wy * vy) / (vx * vx + vy * vy));
  return Math.hypot(px - (ax + vx * t), py - (ay + vy * t));
}

function render(size) {
  const pixels = Buffer.alloc(size * size * 4);
  const samples = 3;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let r = 0,
        g = 0,
        b = 0,
        a = 0;
      for (let sy = 0; sy < samples; sy++) {
        for (let sx = 0; sx < samples; sx++) {
          const px = (x + (sx + 0.5) / samples) / size;
          const py = (y + (sy + 0.5) / samples) / size;

          const plate = roundedBox(px, py, 0.5, 0.5, 0.47, 0.47, 0.22);
          if (plate > 0) continue;

          // background gradient
          const t = clamp01((px * 0.35 + py * 0.75));
          let cr = 0x6d + (0x3d - 0x6d) * t;
          let cg = 0x8f + (0x4b - 0x8f) * t;
          let cb = 0xff + (0xbc - 0xff) * t;
          let alpha = 1;

          const ring = Math.abs(Math.hypot(px - 0.5, py - 0.5) - 0.29) - 0.052;
          const hourHand = distanceToSegment(px, py, 0.5, 0.5, 0.5, 0.285) - 0.043;
          const minuteHand = distanceToSegment(px, py, 0.5, 0.5, 0.705, 0.5) - 0.043;
          const centerDot = Math.hypot(px - 0.5, py - 0.5) - 0.048;

          if (ring < 0 || hourHand < 0 || minuteHand < 0 || centerDot < 0) {
            cr = 255;
            cg = 255;
            cb = 255;
            alpha = 0.97;
          }
          // soft top highlight
          if (py < 0.2) {
            const glow = (0.2 - py) * 0.6;
            cr += (255 - cr) * glow * 0.12;
            cg += (255 - cg) * glow * 0.12;
            cb += (255 - cb) * glow * 0.12;
          }

          r += cr * alpha;
          g += cg * alpha;
          b += cb * alpha;
          a += alpha;
        }
      }
      const total = samples * samples;
      const index = (y * size + x) * 4;
      const cover = a / total;
      if (cover <= 0) continue;
      pixels[index] = Math.round(r / a);
      pixels[index + 1] = Math.round(g / a);
      pixels[index + 2] = Math.round(b / a);
      pixels[index + 3] = Math.round(clamp01(cover) * 255);
    }
  }
  return encodePng(size, size, pixels);
}

function encodeIco(images) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);
  const entries = [];
  let offset = 6 + images.length * 16;
  for (const { size, data } of images) {
    const entry = Buffer.alloc(16);
    entry[0] = size >= 256 ? 0 : size;
    entry[1] = size >= 256 ? 0 : size;
    entry[2] = 0;
    entry[3] = 0;
    entry.writeUInt16LE(1, 4);
    entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(data.length, 8);
    entry.writeUInt32LE(offset, 12);
    entries.push(entry);
    offset += data.length;
  }
  return Buffer.concat([header, ...entries, ...images.map((image) => image.data)]);
}

const sizes = [16, 24, 32, 48, 64, 128, 256, 512];
const rendered = sizes.map((size) => ({ size, data: render(size) }));

for (const { size, data } of rendered) {
  fs.writeFileSync(path.join(OUT, `${size}x${size}.png`), data);
}
fs.writeFileSync(path.join(OUT, "128x128@2x.png"), rendered.find((i) => i.size === 256).data);
fs.writeFileSync(path.join(OUT, "icon.png"), rendered.find((i) => i.size === 512).data);
fs.writeFileSync(
  path.join(OUT, "icon.ico"),
  encodeIco(rendered.filter((item) => item.size <= 256))
);
console.log(`icons written to ${OUT}`);
