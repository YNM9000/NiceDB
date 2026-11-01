import { prepare, type PrepareInput } from './chart';

const scope = self as unknown as { onmessage: ((e: MessageEvent<{ seq: number; input: PrepareInput }>) => void) | null; postMessage: (m: unknown) => void };

scope.onmessage = (e) => scope.postMessage({ seq: e.data.seq, data: prepare(e.data.input) });
