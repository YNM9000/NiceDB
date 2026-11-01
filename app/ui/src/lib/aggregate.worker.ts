import { aggregate, type Column } from './aggregate';

const scope = self as unknown as { onmessage: ((e: MessageEvent<{ seq: number; cols: Column[] }>) => void) | null; postMessage: (m: unknown) => void };

scope.onmessage = (e) => scope.postMessage({ seq: e.data.seq, stats: aggregate(e.data.cols) });
