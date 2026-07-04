// Allocate ~16 MB on the JS heap at module-eval time, then report success.
const chunks = [];
for (let i = 0; i < 4; i++) chunks.push(new Array(1024 * 1024).fill(0));
globalThis.postMessage(`allocated ${chunks.length} chunks`);
