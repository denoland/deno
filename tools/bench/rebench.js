export function benchSync(name, n, innerLoop) {
  const t1 = Date.now();
  for (let i = 0; i < n; i++) {
    innerLoop(i);
  }
  const t2 = Date.now();
  console.log(benchStats(name, n, t1, t2));
}

export async function benchAsync(name, n, innerLoop) {
  const t1 = Date.now();
  for (let i = 0; i < n; i++) {
    await innerLoop(i);
  }
  const t2 = Date.now();
  console.log(benchStats(name, n, t1, t2));
}

function benchStats(name, n, t1, t2) {
  const dt = (t2 - t1) / 1e3;
  const r = n / dt;
  const ns = Math.floor(dt / n * 1e9);
  return `${name}:${" ".repeat(20 - name.length)}\t` +
    `n = ${n}, dt = ${dt.toFixed(3)}s, r = ${r.toFixed(0)}/s, t = ${ns}ns/op`;
}
