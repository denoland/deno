import * as kleur from './clr.mjs';
import { duration } from './fmt.mjs';

export function size(names) {
  let max = 9;
  for (const name of names) if (max < name.length) max = name.length;

  return 2 + max;
}

export function br({ size, avg = true, min_max = true, percentiles = true }) {
  return '-'.repeat(size + 14 * avg + 24 * min_max) + (!percentiles ? '' : ' ' + '-'.repeat(9 + 10 + 10));
}

export function benchmark_error(n, e, { size, avg = true, colors = true, min_max = true, percentiles = true }) {
  return n.padEnd(size, ' ') + `${kleur.red(colors, 'error')}: ${e.message}${e.stack ? '\n' + kleur.gray(colors, e.stack) : ''}`;
}

export function header({ size, avg = true, min_max = true, percentiles = true }) {
  return 'benchmark'.padEnd(size, ' ')
    + (!avg ? '' : 'time (avg)'.padStart(14, ' '))
    + (!min_max ? '' : '(min … max)'.padStart(24, ' '))
    + (!percentiles ? '' : ` ${'p75'.padStart(9, ' ')} ${'p99'.padStart(9, ' ')} ${'p995'.padStart(9, ' ')}`);
}

export function benchmark(n, b, { size, avg = true, colors = true, min_max = true, percentiles = true }) {
  return n.padEnd(size, ' ')
    + (!avg ? '' : `${kleur.yellow(colors, duration(b.avg))}/iter`.padStart(14 + 10 * colors, ' '))
    + (!min_max ? '' : `(${kleur.cyan(colors, duration(b.min))} … ${kleur.magenta(colors, duration(b.max))})`.padStart(24 + 2 * 10 * colors, ' '))
    + (!percentiles ? '' : ` ${kleur.gray(colors, duration(b.p75)).padStart(9 + 10 * colors, ' ')} ${kleur.gray(colors, duration(b.p99)).padStart(9 + 10 * colors, ' ')} ${kleur.gray(colors, duration(b.p995)).padStart(9 + 10 * colors, ' ')}`);
}

export function summary(benchmarks, { colors = true } = {}) {
  benchmarks.sort((a, b) => a.stats.avg - b.stats.avg);
  const baseline = benchmarks.find(b => b.baseline) || benchmarks[0];

  return kleur.bold(colors, 'summary')
    + `\n  ${kleur.bold(colors, kleur.cyan(colors, baseline.name))}`

    + benchmarks.filter(b => b !== baseline).map(b => {
      const diff = Number((1 / baseline.stats.avg * b.stats.avg).toFixed(2));
      const inv_diff = Number((1 / b.stats.avg * baseline.stats.avg).toFixed(2));
      return `\n   ${kleur[1 > diff ? 'red' : 'green'](colors, 1 <= diff ? diff : inv_diff)}x ${1 > diff ? 'slower' : 'faster'} than ${kleur.bold(colors, kleur.cyan(colors, b.name))}`;
    }).join('');
}