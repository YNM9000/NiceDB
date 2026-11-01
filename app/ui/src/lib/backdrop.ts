export type Rgb = [number, number, number];

export interface BackdropColors {
  surface: Rgb;
  raised: Rgb;
  ink: Rgb;
}

const mix = (a: Rgb, b: Rgb, wa: number): Rgb => [a[0] * wa + b[0] * (1 - wa), a[1] * wa + b[1] * (1 - wa), a[2] * wa + b[2] * (1 - wa)];

const toLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toGamma = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * Math.max(c, 0) ** (1 / 2.4) - 0.055);

export function srgbToOklab([r, g, b]: Rgb): Rgb {
  const [lr, lg, lb] = [toLinear(r / 255), toLinear(g / 255), toLinear(b / 255)];
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}

export function oklabToSrgb([L, a, b]: Rgb): Rgb {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    255 * toGamma(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    255 * toGamma(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    255 * toGamma(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
  ];
}

export function parseColor(text: string): Rgb | null {
  const rgb = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)/.exec(text);
  if (rgb) return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])];
  const srgb = /^color\(srgb\s+([\d.e-]+)\s+([\d.e-]+)\s+([\d.e-]+)/.exec(text);
  if (srgb) return [Number(srgb[1]) * 255, Number(srgb[2]) * 255, Number(srgb[3]) * 255];
  return null;
}

const DITHER = (() => {
  const t = new Float32Array(64 * 64);
  for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) t[y * 64 + x] = ((52.9829189 * ((0.06711056 * x + 0.00583715 * y) % 1)) % 1) - 0.5;
  return t;
})();

const STEPS = 1024;

export function paintBackdrop(w: number, h: number, c: BackdropColors, out: Uint8ClampedArray<ArrayBuffer> = new Uint8ClampedArray(w * h * 4)): Uint8ClampedArray<ArrayBuffer> {
  const glow = mix(c.surface, c.raised, 0.92);
  const rim = srgbToOklab(mix(c.surface, c.ink, 0.88));
  const base = srgbToOklab(c.surface);
  const ramp = new Float32Array((STEPS + 2) * 3);
  for (let i = 0; i <= STEPS + 1; i++) {
    const u = Math.min(i / STEPS, 1);
    const p = oklabToSrgb([base[0] + (rim[0] - base[0]) * u, base[1] + (rim[1] - base[1]) * u, base[2] + (rim[2] - base[2]) * u]);
    ramp.set(p, i * 3);
  }
  const cx = w / 2;
  const gx = new Float32Array(w);
  const rx = new Float32Array(w);
  for (let x = 0; x < w; x++) {
    const d = x + 0.5 - cx;
    gx[x] = (d / (0.6 * w)) ** 2;
    rx[x] = (d / (1.2 * w)) ** 2;
  }
  let p = 0;
  for (let y = 0; y < h; y++) {
    const gy = ((y + 0.5 - 0.46 * h) / (0.55 * h)) ** 2;
    const ry = ((y + 0.5 - 0.45 * h) / h) ** 2;
    const row = (y & 63) * 64;
    for (let x = 0; x < w; x++) {
      const tg = Math.sqrt(gx[x]! + gy);
      const u = Math.min(Math.max((Math.sqrt(rx[x]! + ry) - 0.4) / 0.6, 0), 1) * STEPS;
      const i = u | 0;
      const f = u - i;
      const k = i * 3;
      const a = tg < 0.7 ? 1 - tg / 0.7 : 0;
      const b = 1 - a;
      const n = DITHER[row + (x & 63)]!;
      const r0 = ramp[k]!, g0 = ramp[k + 1]!, b0 = ramp[k + 2]!;
      out[p] = (r0 + (ramp[k + 3]! - r0) * f) * b + glow[0] * a + n;
      out[p + 1] = (g0 + (ramp[k + 4]! - g0) * f) * b + glow[1] * a + n;
      out[p + 2] = (b0 + (ramp[k + 5]! - b0) * f) * b + glow[2] * a + n;
      out[p + 3] = 255;
      p += 4;
    }
  }
  return out;
}
