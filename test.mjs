import { performance } from 'node:perf_hooks';
function someFunction() { console.log('hello world'); }
const wrapped = performance.timerify(someFunction);
wrapped();
