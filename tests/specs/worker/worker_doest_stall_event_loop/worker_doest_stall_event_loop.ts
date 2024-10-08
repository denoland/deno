const moduleCode = `
console.log('module start');
const hash = await crypto.subtle.digest('SHA-1', new TextEncoder().encode('data'));
const __default = {};
export { __default as default };
console.log('module finish');
`;

const workerCode = `
    console.log('worker!');

    globalThis.onmessage = (msg) => {
        const { moduleCode } = msg.data;
        (async () => {
            console.log('before import');
            await import(URL.createObjectURL(new Blob([ moduleCode ])));
            console.log('after import');
            self.postMessage('thanks');
        })();
    }
`;
const worker = new Worker(URL.createObjectURL(new Blob([workerCode])), {
  type: "module",
});
worker.onmessage = () => {
  console.log("worker.terminate");
  worker.terminate();
};
worker.postMessage({ moduleCode });
