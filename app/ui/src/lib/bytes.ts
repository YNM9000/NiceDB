const CHUNK = 0x8000;

export function toBase64(data: Uint8Array): string {
  const parts: string[] = [];
  for (let i = 0; i < data.length; i += CHUNK) parts.push(String.fromCharCode(...data.subarray(i, i + CHUNK)));
  return btoa(parts.join(''));
}

export function fromBase64(data: string): Uint8Array {
  const raw = atob(data);
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
  return out;
}
