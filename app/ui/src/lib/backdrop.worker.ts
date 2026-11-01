import { paintBackdrop, type BackdropColors } from './backdrop';

interface Job {
  seq: number;
  w: number;
  h: number;
  colors: BackdropColors;
}

const scope = self as unknown as { onmessage: ((e: MessageEvent<Job>) => void) | null; postMessage: (m: unknown, transfer: Transferable[]) => void };

scope.onmessage = async ({ data: { seq, w, h, colors } }) => {
  const px = paintBackdrop(w, h, colors);
  const image = new ImageData(px, w, h);
  if (typeof createImageBitmap === 'function') {
    const bitmap = await createImageBitmap(image);
    scope.postMessage({ seq, bitmap }, [bitmap]);
  } else scope.postMessage({ seq, w, h, px }, [px.buffer]);
};
