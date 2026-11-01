import { bounds, routeAll, routeNear, type ErTable, type Point, type Rect } from './model';
import { coords, draw, sheet, drawMinimap, drawViewport, fitView, hitTable, miniFrame, NO_MARKS, type Look, type Marks, type MiniFrame, type Scene, type View } from './render';

export interface StageEvents {
  select: (id: string | null) => void;
  open: (id: string) => void;
  moved: () => void;
  view: (v: View, whole: boolean) => void;
  menu?: (id: string | null, e: MouseEvent) => void;
}

type Drag =
  | { kind: 'pan'; x: number; y: number; moved: boolean; deselect: boolean }
  | { kind: 'table'; t: ErTable; x: number; y: number; ox: number; oy: number; moved: boolean }
  | { kind: 'pinch'; d: number; k: number; c: Point }
  | { kind: 'mini' };

const K_MAX = 3;
const SNAP = 4;

const reducedMotion = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);

export class Stage {
  view: View = { x: 0, y: 0, k: 1 };
  private scene: Scene | null = null;
  private look: Look | null = null;
  private marks: Marks = NO_MARKS;
  private ctx: CanvasRenderingContext2D | null;
  private miniCtx: CanvasRenderingContext2D | null;
  private miniCache = document.createElement('canvas');
  private miniDirty = true;
  private mini: MiniFrame = { scale: 1, ox: 0, oy: 0 };
  private origin: Point = { x: 0, y: 0 };
  private w = 0;
  private h = 0;
  private dpr = 1;
  private frame = 0;
  private flight = 0;
  private drag: Drag | null = null;
  private pointers = new Map<number, Point>();
  private space = false;
  private resize: ResizeObserver;
  private off: (() => void)[] = [];

  constructor(
    private host: HTMLElement,
    private canvas: HTMLCanvasElement,
    private miniCanvas: HTMLCanvasElement,
    private ev: StageEvents,
  ) {
    this.ctx = canvas.getContext('2d', { alpha: false });
    this.miniCtx = miniCanvas.getContext('2d');
    this.resize = new ResizeObserver(() => this.measure());
    this.resize.observe(host);
    this.resize.observe(miniCanvas);
    this.listen(canvas, 'pointerdown', (e) => this.down(e));
    this.listen(canvas, 'pointermove', (e) => this.move(e));
    this.listen(canvas, 'pointerup', (e) => this.up(e));
    this.listen(canvas, 'pointercancel', (e) => this.up(e));
    this.listen(canvas, 'dblclick', (e) => this.dbl(e));
    this.listen(canvas, 'contextmenu', (e) => this.context(e));
    this.listen(canvas, 'wheel', (e) => this.wheel(e), { passive: false });
    this.listen(host, 'keydown', (e) => this.key(e, true));
    this.listen(host, 'keyup', (e) => this.key(e, false));
    this.listen(host, 'blur', () => (this.space = false));
    this.listen(miniCanvas, 'pointerdown', (e) => this.miniDown(e));
    this.listen(miniCanvas, 'pointermove', (e) => this.drag?.kind === 'mini' && this.miniAt(e));
    this.listen(miniCanvas, 'pointerup', () => (this.drag = null));
    this.measure();
  }

  private listen<K extends keyof HTMLElementEventMap>(el: HTMLElement, type: K, fn: (e: HTMLElementEventMap[K]) => void, opts?: AddEventListenerOptions) {
    el.addEventListener(type, fn, opts);
    this.off.push(() => el.removeEventListener(type, fn, opts));
  }

  destroy() {
    cancelAnimationFrame(this.frame);
    cancelAnimationFrame(this.flight);
    this.resize.disconnect();
    for (const f of this.off) f();
  }

  get size() {
    return { w: this.w, h: this.h };
  }

  setScene(s: Scene | null) {
    this.scene = s;
    this.relayout();
  }

  relayout() {
    if (this.scene) {
      const b = bounds(this.scene.graph.tables, 60);
      this.origin = { x: b.x, y: b.y };
    }
    this.miniDirty = true;
    this.invalidate();
  }

  setLook(l: Look) {
    this.look = l;
    this.miniDirty = true;
    this.invalidate();
  }

  setMarks(m: Marks) {
    this.marks = m;
    this.miniDirty = true;
    this.invalidate();
  }

  private measure() {
    const r = this.host.getBoundingClientRect();
    this.dpr = window.devicePixelRatio || 1;
    this.w = Math.max(1, Math.round(r.width));
    this.h = Math.max(1, Math.round(r.height));
    this.canvas.width = Math.round(this.w * this.dpr);
    this.canvas.height = Math.round(this.h * this.dpr);
    const mw = this.miniCanvas.clientWidth;
    const mh = this.miniCanvas.clientHeight;
    this.miniCanvas.width = Math.round(mw * this.dpr);
    this.miniCanvas.height = Math.round(mh * this.dpr);
    this.miniDirty = true;
    this.paint();
  }

  invalidate() {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.paint();
    });
  }

  private paint() {
    const { ctx, scene, look } = this;
    if (!ctx || !look) return;
    if (!scene) {
      ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
      sheet(ctx, look, this.view, this.w, this.h);
      return;
    }
    const started = performance.now();
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    draw(ctx, scene, look, this.view, this.w, this.h, this.marks);
    coords(ctx, look, this.view, this.w, this.h, this.origin);
    this.paintMini();
    (globalThis as { __ndErDraw?: number[] }).__ndErDraw?.push(performance.now() - started);
    this.ev.view(this.view, this.whole());
  }

  private whole(): boolean {
    if (!this.scene) return true;
    const b = bounds(this.scene.graph.tables.filter((t) => !this.marks.hidden.has(t.id)));
    const v = this.viewport();
    const slack = 4 / this.view.k;
    return b.x >= v.x - slack && b.y >= v.y - slack && b.x + b.w <= v.x + v.w + slack && b.y + b.h <= v.y + v.h + slack;
  }

  private paintMini() {
    const { miniCtx: ctx, scene, look } = this;
    if (!ctx || !scene || !look) return;
    const w = this.miniCanvas.clientWidth;
    const h = this.miniCanvas.clientHeight;
    if (w === 0 || h === 0) return;
    if (this.miniDirty) {
      this.mini = miniFrame(scene, this.marks.hidden, w, h);
      this.miniCache.width = this.miniCanvas.width;
      this.miniCache.height = this.miniCanvas.height;
      const c = this.miniCache.getContext('2d');
      if (c) {
        c.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
        drawMinimap(c, scene, look, w, h, this.marks, this.mini);
      }
      this.miniDirty = false;
    }
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, this.miniCanvas.width, this.miniCanvas.height);
    ctx.drawImage(this.miniCache, 0, 0);
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    drawViewport(ctx, look, this.mini, this.viewport());
  }

  viewport(): Rect {
    const { x, y, k } = this.view;
    return { x: -x / k, y: -y / k, w: this.w / k, h: this.h / k };
  }

  private world(sx: number, sy: number): Point {
    return { x: (sx - this.view.x) / this.view.k, y: (sy - this.view.y) / this.view.k };
  }

  private local(e: MouseEvent): Point {
    const r = this.canvas.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  private kMin() {
    if (!this.scene) return 0.05;
    const b = bounds(this.scene.graph.tables, 40);
    return Math.min(0.2, fitView(b, this.w, this.h).k * 0.6);
  }

  setView(v: View) {
    cancelAnimationFrame(this.flight);
    const k = Math.min(K_MAX, Math.max(this.kMin(), v.k));
    const cx = this.w / 2;
    const cy = this.h / 2;
    this.view = k === v.k ? v : { k, x: cx - ((cx - v.x) / v.k) * k, y: cy - ((cy - v.y) / v.k) * k };
    this.invalidate();
  }

  zoomAt(p: Point, factor: number) {
    const k = Math.min(K_MAX, Math.max(this.kMin(), this.view.k * factor));
    const w = this.world(p.x, p.y);
    this.setView({ k, x: p.x - w.x * k, y: p.y - w.y * k });
  }

  zoomBy(factor: number) {
    this.animate(this.zoomed({ x: this.w / 2, y: this.h / 2 }, factor));
  }

  zoomTo(k: number) {
    this.animate(this.zoomed({ x: this.w / 2, y: this.h / 2 }, k / this.view.k));
  }

  private zoomed(p: Point, factor: number): View {
    const k = Math.min(K_MAX, Math.max(this.kMin(), this.view.k * factor));
    const w = this.world(p.x, p.y);
    return { k, x: p.x - w.x * k, y: p.y - w.y * k };
  }

  fit(animate = true) {
    if (!this.scene) return;
    this.fitTo(this.scene.graph.tables.filter((t) => !this.marks.hidden.has(t.id)), animate);
  }

  fitTo(shown: readonly ErTable[], animate = true) {
    if (shown.length === 0) return;
    const i = this.look?.inset ?? { top: 0, right: 0, bottom: 0, left: 0 };
    const v = fitView(bounds(shown), this.w - i.left - i.right, this.h - i.top - i.bottom, 40);
    const target = { k: v.k, x: v.x + i.left, y: v.y + i.top };
    if (animate) this.animate(target);
    else {
      this.view = target;
      this.invalidate();
    }
  }

  flyTo(t: ErTable) {
    const fits = Math.min((this.h - 80) / t.h, (this.w - 80) / t.w);
    const k = Math.max(this.kMin(), Math.min(fits, Math.max(this.view.k, 0.85), 1.1));
    this.animate({ k, x: this.w / 2 - (t.x + t.w / 2) * k, y: this.h / 2 - (t.y + t.h / 2) * k });
  }

  private animate(to: View) {
    cancelAnimationFrame(this.flight);
    if (reducedMotion()) {
      this.view = to;
      this.invalidate();
      return;
    }
    const from = { ...this.view };
    const start = performance.now();
    const dur = 360;
    const step = (now: number) => {
      const t = ease(Math.min(1, (now - start) / dur));
      const lk = Math.log(from.k) + (Math.log(to.k) - Math.log(from.k)) * t;
      const k = Math.exp(lk);
      const cx = this.w / 2;
      const cy = this.h / 2;
      const fc = { x: (cx - from.x) / from.k, y: (cy - from.y) / from.k };
      const tc = { x: (cx - to.x) / to.k, y: (cy - to.y) / to.k };
      const wc = { x: fc.x + (tc.x - fc.x) * t, y: fc.y + (tc.y - fc.y) * t };
      this.view = { k, x: cx - wc.x * k, y: cy - wc.y * k };
      this.paint();
      if (t < 1) this.flight = requestAnimationFrame(step);
    };
    this.flight = requestAnimationFrame(step);
  }

  private down(e: PointerEvent) {
    if (!this.scene) return;
    cancelAnimationFrame(this.flight);
    this.host.focus({ preventScroll: true });
    const p = this.local(e);
    this.pointers.set(e.pointerId, p);
    this.canvas.setPointerCapture(e.pointerId);
    if (this.pointers.size === 2) {
      const [a, b] = [...this.pointers.values()];
      if (a && b) this.drag = { kind: 'pinch', d: Math.hypot(a.x - b.x, a.y - b.y), k: this.view.k, c: this.world((a.x + b.x) / 2, (a.y + b.y) / 2) };
      return;
    }
    if (e.button === 1 || (e.button === 0 && this.space)) {
      this.drag = { kind: 'pan', x: p.x, y: p.y, moved: false, deselect: false };
      this.canvas.style.cursor = 'grabbing';
      return;
    }
    if (e.button !== 0) return;
    const t = hitTable(this.scene.graph, this.marks.hidden, this.world(p.x, p.y));
    this.drag = t ? { kind: 'table', t, x: p.x, y: p.y, ox: t.x, oy: t.y, moved: false } : { kind: 'pan', x: p.x, y: p.y, moved: false, deselect: true };
  }

  private move(e: PointerEvent) {
    const p = this.local(e);
    if (this.pointers.has(e.pointerId)) this.pointers.set(e.pointerId, p);
    const d = this.drag;
    if (!d) {
      this.hover(p);
      return;
    }
    if (d.kind === 'pinch') {
      const [a, b] = [...this.pointers.values()];
      if (!a || !b) return;
      const k = Math.min(K_MAX, Math.max(this.kMin(), (d.k * Math.hypot(a.x - b.x, a.y - b.y)) / Math.max(1, d.d)));
      const m = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
      this.view = { k, x: m.x - d.c.x * k, y: m.y - d.c.y * k };
      this.invalidate();
      return;
    }
    if (d.kind === 'mini') return;
    const dx = p.x - d.x;
    const dy = p.y - d.y;
    if (!d.moved && Math.hypot(dx, dy) < 3) return;
    d.moved = true;
    if (d.kind === 'pan') {
      this.canvas.style.cursor = 'grabbing';
      this.view = { ...this.view, x: this.view.x + dx, y: this.view.y + dy };
      d.x = p.x;
      d.y = p.y;
      this.invalidate();
      return;
    }
    const scene = this.scene;
    if (!scene) return;
    d.t.x = Math.round((d.ox + dx / this.view.k) / SNAP) * SNAP;
    d.t.y = Math.round((d.oy + dy / this.view.k) / SNAP) * SNAP;
    routeAll(scene.graph, new Set([d.t.id]));
    this.canvas.style.cursor = 'move';
    this.miniDirty = true;
    this.invalidate();
  }

  private up(e: PointerEvent) {
    this.pointers.delete(e.pointerId);
    const d = this.drag;
    this.drag = null;
    this.canvas.style.cursor = '';
    if (!d) return;
    if (d.kind === 'table') {
      if (d.moved) {
        if (this.scene) routeNear(this.scene.graph, d.t.id);
        this.relayout();
        this.ev.moved();
      } else this.ev.select(d.t.id);
    } else if (d.kind === 'pan' && !d.moved && d.deselect) this.ev.select(null);
  }

  private hover(p: Point) {
    if (!this.scene) return;
    const t = hitTable(this.scene.graph, this.marks.hidden, this.world(p.x, p.y));
    this.canvas.style.cursor = this.space ? 'grab' : t ? 'pointer' : '';
  }

  private dbl(e: MouseEvent) {
    if (!this.scene) return;
    const p = this.local(e);
    const t = hitTable(this.scene.graph, this.marks.hidden, this.world(p.x, p.y));
    if (t) this.ev.open(t.id);
  }

  private context(e: MouseEvent) {
    if (!this.scene || !this.ev.menu) return;
    const p = this.local(e);
    const t = hitTable(this.scene.graph, this.marks.hidden, this.world(p.x, p.y));
    if (t) this.ev.select(t.id);
    this.ev.menu(t?.id ?? null, e);
  }

  private wheel(e: WheelEvent) {
    e.preventDefault();
    cancelAnimationFrame(this.flight);
    const scale = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1;
    const dy = e.deltaY * scale;
    if (e.shiftKey && !e.ctrlKey) {
      this.view = { ...this.view, x: this.view.x - dy };
      this.invalidate();
      return;
    }
    this.zoomAt(this.local(e), Math.exp(-dy * (e.ctrlKey ? 0.01 : 0.0018)));
  }

  private key(e: KeyboardEvent, down: boolean) {
    if (e.target !== this.host) return;
    if (e.key === ' ') {
      e.preventDefault();
      this.space = down;
      this.canvas.style.cursor = down ? 'grab' : '';
      return;
    }
    if (!down || e.ctrlKey || e.metaKey || e.altKey) return;
    const pan = (x: number, y: number) => {
      e.preventDefault();
      this.setView({ ...this.view, x: this.view.x + x, y: this.view.y + y });
    };
    switch (e.key) {
      case '+':
      case '=':
        this.zoomBy(1.25);
        break;
      case '-':
        this.zoomBy(0.8);
        break;
      case '0':
        this.zoomTo(1);
        break;
      case 'f':
      case 'F':
        this.fit();
        break;
      case 'ArrowLeft':
        pan(80, 0);
        break;
      case 'ArrowRight':
        pan(-80, 0);
        break;
      case 'ArrowUp':
        pan(0, 80);
        break;
      case 'ArrowDown':
        pan(0, -80);
        break;
      case 'Escape':
        this.ev.select(null);
        break;
      default:
        return;
    }
  }

  private miniDown(e: PointerEvent) {
    this.miniCanvas.setPointerCapture(e.pointerId);
    this.drag = { kind: 'mini' };
    this.miniAt(e);
  }

  private miniAt(e: PointerEvent) {
    const r = this.miniCanvas.getBoundingClientRect();
    const wx = (e.clientX - r.left - this.mini.ox) / this.mini.scale;
    const wy = (e.clientY - r.top - this.mini.oy) / this.mini.scale;
    this.setView({ ...this.view, x: this.w / 2 - wx * this.view.k, y: this.h / 2 - wy * this.view.k });
  }
}
