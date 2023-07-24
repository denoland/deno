// https://github.com/denoland/deno/issues/19903
// https://github.com/denoland/deno/issues/19455

// The worker here is not important, but the existence of a worker appears to be causing the hang

// If this timer triggers, we locked up
const timeout = setTimeout(() => {
  Deno.exit(1);
}, 10000);
Deno.unrefTimer(timeout);

const workerCode = `
console.log('worker!');
`;
const worker = new Worker(URL.createObjectURL(new Blob([ workerCode ])), { type: 'module' });

const moduleCode = `
console.log('module start');
const hash = await crypto.subtle.digest('SHA-1', new TextEncoder().encode('data'));
const __default = {};
export { __default as default };
console.log('module finish');
`;
console.log('before import');
await import(URL.createObjectURL(new Blob([ moduleCode ])));
console.log('after import');

worker.terminate();
