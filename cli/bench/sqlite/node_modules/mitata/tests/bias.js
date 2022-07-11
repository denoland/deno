import { run, bench, group, baseline } from '..';

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

await run({ percentiles: false });