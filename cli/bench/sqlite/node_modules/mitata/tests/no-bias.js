import { run, bench, group, baseline } from '..';

const fn = () => {};

bench('noop', fn);
bench('noop', fn);
bench('noop', fn);
bench('noop', fn);
bench('noop', fn);
bench('noop', fn);
bench('noop', fn);
bench('noop', fn);
bench('noop', fn);
bench('noop', fn);
bench('noop', fn);
bench('noop', fn);
bench('noop', fn);

await run({ percentiles: false });