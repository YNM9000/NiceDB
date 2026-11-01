import { parseColor, type BackdropColors, type Rgb } from './backdrop';

type Reply = { seq: number; bitmap: ImageBitmap } | { seq: number; w: number; h: number; px: Uint8ClampedArray<ArrayBuffer> };

function token(host: HTMLElement, name: string): Rgb {
  const probe = host.appendChild(document.createElement('i'));
  probe.style.color = `var(${name})`;
  const text = getComputedStyle(probe).color;
  probe.remove();
  const parsed = parseColor(text);
  if (parsed) return parsed;
  const ctx = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
  if (!ctx) return [0, 0, 0];
  ctx.fillStyle = text;
  ctx.fillRect(0, 0, 1, 1);
  const [r = 0, g = 0, b = 0] = ctx.getImageData(0, 0, 1, 1).data;
  return [r, g, b];
}

export function backdropColors(host: HTMLElement): BackdropColors {
  return { surface: token(host, '--nd-surface'), raised: token(host, '--nd-raised'), ink: token(host, '--nd-ink') };
}

export function smoothBackdrop(canvas: HTMLCanvasElement, host: HTMLElement): () => void {
  if (typeof Worker === 'undefined') return () => {};
  const worker = new Worker(new URL('./backdrop.worker.ts', import.meta.url), { type: 'module' });
  let seq = 0;
  let started = 0;
  let timer = 0;
  worker.onmessage = ({ data }: MessageEvent<Reply>) => {
    if (data.seq !== seq) {
      if ('bitmap' in data) data.bitmap.close();
      return;
    }
    if ('bitmap' in data) {
      canvas.width = data.bitmap.width;
      canvas.height = data.bitmap.height;
      canvas.getContext('2d')?.drawImage(data.bitmap, 0, 0);
      data.bitmap.close();
    } else {
      canvas.width = data.w;
      canvas.height = data.h;
      canvas.getContext('2d')?.putImageData(new ImageData(data.px, data.w, data.h), 0, 0);
    }
    host.dataset.smooth = '';
    performance.measure('nd-splash-backdrop', { start: started });
  };
  const paint = () => {
    const dpr = devicePixelRatio || 1;
    started = performance.now();
    worker.postMessage({ seq: ++seq, w: Math.max(1, Math.round(host.clientWidth * dpr)), h: Math.max(1, Math.round(host.clientHeight * dpr)), colors: backdropColors(host) });
  };
  const later = () => {
    clearTimeout(timer);
    timer = window.setTimeout(paint, 120);
  };
  const recolor = () => {
    delete host.dataset.smooth;
    later();
  };
  paint();
  addEventListener('resize', later);
  const watch = new MutationObserver(recolor);
  watch.observe(document.documentElement, { attributes: true, attributeFilter: ['data-style', 'data-theme', 'data-palette'] });
  return () => {
    clearTimeout(timer);
    removeEventListener('resize', later);
    watch.disconnect();
    worker.terminate();
  };
}
