export function siblingPositions(depths: readonly number[]): { pos: number[]; size: number[] } {
  const pos: number[] = new Array<number>(depths.length).fill(1);
  const size: number[] = new Array<number>(depths.length).fill(1);
  const open: (number[] | undefined)[] = [];
  const close = (from: number) => {
    for (let d = open.length - 1; d >= from; d--) for (const i of open[d] ?? []) size[i] = open[d]?.length ?? 1;
    open.length = Math.min(open.length, from);
  };
  depths.forEach((d, i) => {
    close(d + 1);
    const group = (open[d] ??= []);
    group.push(i);
    pos[i] = group.length;
  });
  close(0);
  return { pos, size };
}
