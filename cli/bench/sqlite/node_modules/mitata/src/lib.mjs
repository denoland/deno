import * as time from './time.mjs';

const now = time.now;

function sort(a, b) {
  if (a > b) return 1;
  if (a < b) return -1;

  return 0;
};

function stats(n, t, avg, min, max, jit, all) {
  return {
    n, min, max, jit,
    p75: all[Math.ceil(n * (75 / 100)) - 1],
    p99: all[Math.ceil(n * (99 / 100)) - 1],
    avg: !t ? (avg / n) :  Math.ceil(avg / n),
    p995: all[Math.ceil(n * (99.5 / 100)) - 1],
    p999: all[Math.ceil(n * (99.9 / 100)) - 1],
  };
}

export function sync(t, fn, collect = false) {
  let n = 0;
  let avg = 0;
  let wavg = 0;
  let min = Infinity;
  let max = -Infinity;
  const all = new Array;
  const jit = new Array(10);

  warmup: {
    let offset = 0;
    let iterations = 10;
    while (iterations--) {
      const t1 = now();

      fn();
      jit[offset++] = time.diff(now(), t1);
    }

    let c = 0;
    iterations = 4;
    let budget = 10 * 1e6;

    while (0 < budget || 0 < iterations--) {
      const t1 = now();

      fn();
      const t2 = time.diff(now(), t1);
      if (0 > t2) { iterations++; continue; };

      c++;
      wavg += t2;
      budget -= t2;
    }

    wavg /= c;
  }

  measure: {
    if (wavg > 10_000) {
      let iterations = 10;
      let budget = t * 1e6;

      while (0 < budget || 0 < iterations--) {
        const t1 = now();

        fn();
        const t2 = time.diff(now(), t1);
        if (0 > t2) { iterations++; continue; };

        n++;
        avg += t2;
        budget -= t2;
        all.push(t2);
        if (t2 < min) min = t2;
        if (t2 > max) max = t2;
      }
    }

    else {
      let iterations = 10;
      let budget = t * 1e6;

      if (!collect) while (0 < budget || 0 < iterations--) {
        const t1 = now();
        for (let c = 0; c < 1e4; c++) fn();
        const t2 = time.diff(now(), t1) / 1e4;
        if (0 > t2) { iterations++; continue; };

        n++;
        avg += t2;
        all.push(t2);
        budget -= t2 * 1e4;
        if (t2 < min) min = t2;
        if (t2 > max) max = t2;
      }

      else {
        const garbage = new Array(1e4);

        while (0 < budget || 0 < iterations--) {
          const t1 = now();
          for (let c = 0; c < 1e4; c++) garbage[c] = fn();

          const t2 = time.diff(now(), t1) / 1e4;
          if (0 > t2) { iterations++; continue; };
  
          n++;
          avg += t2;
          all.push(t2);
          budget -= t2 * 1e4;
          if (t2 < min) min = t2;
          if (t2 > max) max = t2;
        }
      }
    }
  }

  all.sort(sort);
  return stats(n, wavg > 10_000, avg, min, max, jit, all);
}

export async function async(t, fn, collect = false) {
  let n = 0;
  let avg = 0;
  let wavg = 0;
  let min = Infinity;
  let max = -Infinity;
  const all = new Array;
  const jit = new Array(10);

  warmup: {
    let offset = 0;
    let iterations = 10;
    while (iterations--) {
      const t1 = now();

      await fn();
      jit[offset++] = time.diff(now(), t1);
    }

    let c = 0;
    iterations = 4;
    let budget = 10 * 1e6;

    while (0 < budget || 0 < iterations--) {
      const t1 = now();

      await fn();
      const t2 = time.diff(now(), t1);
      if (0 > t2) { iterations++; continue; };

      c++;
      wavg += t2;
      budget -= t2;
    }

    wavg /= c;
  }

  measure: {
    if (wavg > 10_000) {
      let iterations = 10;
      let budget = t * 1e6;

      while (0 < budget || 0 < iterations--) {
        const t1 = now();

        await fn();
        const t2 = time.diff(now(), t1);
        if (0 > t2) { iterations++; continue; };

        n++;
        avg += t2;
        budget -= t2;
        all.push(t2);
        if (t2 < min) min = t2;
        if (t2 > max) max = t2;
      }
    }

    else {
      let iterations = 10;
      let budget = t * 1e6;

      if (!collect) while (0 < budget || 0 < iterations--) {
        const t1 = now();
        for (let c = 0; c < 1e4; c++) await fn();

        const t2 = time.diff(now(), t1) / 1e4;
        if (0 > t2) { iterations++; continue; };

        n++;
        avg += t2;
        all.push(t2);
        budget -= t2 * 1e4;
        if (t2 < min) min = t2;
        if (t2 > max) max = t2;
      }

      else {
        const garbage = new Array(1e4);

        while (0 < budget || 0 < iterations--) {
          const t1 = now();
          for (let c = 0; c < 1e4; c++) garbage[c] = await fn();

          const t2 = time.diff(now(), t1) / 1e4;
          if (0 > t2) { iterations++; continue; };
  
          n++;
          avg += t2;
          all.push(t2);
          budget -= t2 * 1e4;
          if (t2 < min) min = t2;
          if (t2 > max) max = t2;
        }
      }
    }
  }

  all.sort(sort);
  return stats(n, wavg > 10_000, avg, min, max, jit, all);
}