const time = (() => {
  const ceil = Math.ceil;

  try {
    Bun.nanoseconds();

    return {
      now: Bun.nanoseconds,
      diff: (a, b) => a - b,
    };
  } catch { }

  try {
    process.hrtime.bigint();
    if ('Deno' in globalThis) throw 0;

    return {
      now: process.hrtime.bigint,
      diff: (a, b) => Number(a - b),
    };
  } catch { }

  try {
    Deno.core.opSync('op_bench_now');

    return {
      diff: (a, b) => a - b,
      now: () => Deno.core.opSync('op_bench_now'),
    };
  } catch { }

  try {
    Deno.core.opSync('op_now');

    return {
      diff: (a, b) => a - b,
      now: () => ceil(1e6 * Deno.core.opSync('op_now')),
    };
  } catch { }


  return {
    diff: (a, b) => a - b,
    now: () => ceil(1e6 * performance.now()),
  };
})();

export const now = time.now;
export const diff = time.diff;