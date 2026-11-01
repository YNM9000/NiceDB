const MAX_PIXELS = 40_000_000;

export async function tiffPreview(bytes: Uint8Array): Promise<{ url: string; width: number; height: number } | null> {
  try {
    const { default: UTIF } = await import('utif');
    const buffer = bytes.slice().buffer;
    const ifds = UTIF.decode(buffer);
    const first = ifds[0];
    if (!first) return null;
    UTIF.decodeImage(buffer, first, ifds);
    const { width, height } = first;
    if (!width || !height || width * height > MAX_PIXELS) return null;
    const rgba = UTIF.toRGBA8(first);
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) return null;
    ctx.putImageData(new ImageData(Uint8ClampedArray.from(rgba.subarray(0, width * height * 4)), width, height), 0, 0);
    return { url: canvas.toDataURL('image/png'), width, height };
  } catch {
    return null;
  }
}
