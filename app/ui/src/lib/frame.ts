export type FrameKind = 'linux' | 'windows' | 'macos' | null;

export function detectFrame(ua: string, tauri: boolean, search = ''): FrameKind {
  const forced = new URLSearchParams(search).get('frame');
  if (forced === 'linux' || forced === 'windows' || forced === 'macos') return forced;
  if (!tauri) return null;
  if (/Macintosh|Mac OS X/.test(ua)) return 'macos';
  if (/Windows/.test(ua)) return 'windows';
  return 'linux';
}
