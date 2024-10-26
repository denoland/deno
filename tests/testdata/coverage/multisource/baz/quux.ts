export function quux(cond: boolean) {
  if (cond) {
    const a = 1;
    const b = a;
    const c = b;
    const d = c;
    const e = d;
    const f = e;
    const g = f;
    return g;
  } else {
    return 2;
  }
}
