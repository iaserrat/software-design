function clampIndex(index: number, length: number): number { return Math.max(0, Math.min(index, length)); }
export function first(xs: number[], i: number): number { return xs[clampIndex(i, xs.length)]; }
export function last(xs: number[], i: number): number { return xs[clampIndex(xs.length - i, xs.length)]; }
