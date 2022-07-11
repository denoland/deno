<h1 align=center>mitata</h1>
<div align=center>cross-runtime benchmarking lib</div>

<br />

### Install
`bun add mitata`

`npm install mitata`

## Examples
```js
import { run, bench, group, baseline } from 'mitata';

// deno
// import { ... } from 'https://esm.sh/mitata';

bench('noop', () => {});
bench('noop2', () => {});

group('group', () => {
  baseline('baseline', () => {});
  bench('Date.now()', () => Date.now());
  bench('performance.now()', () => performance.now());
});

group({ name: 'group2', summary: false }, () => {
  bench('new Array(0)', () => new Array(0));
  bench('new Array(1024)', () => new Array(1024));
});

await run({
  avg: true, // enable/disable avg column (default: true)
  json: false, // enable/disable json output (default: false)
  colors: true, // enable/disable colors (default: true)
  min_max: true, // enable/disable min/max column (default: true)
  collect: false, // enable/disable collecting returned values into an array during the benchmark (default: false)
  percentiles: false, // enable/disable percentiles column (default: true)
});
```

![preview.png](https://cdn.discordapp.com/attachments/640955857038999552/973337478050230312/unknown.png)

## JIT bias
If you run benchmarks like this, you might notice that they get slower (only few nanoseconds) after the first few runs.

```js
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
bench('noop', () => {});
```

I call this behavior "JIT bias". In simple words, v8 and JSC JIT expect us to pass the same function, so they optimize for it, but we break that promise and get deoptimized.

## License

MIT © [Evan](https://github.com/evanwashere)