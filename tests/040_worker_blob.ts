const b = new Blob(["console.log('code from Blob'); postMessage('DONE')"]);
const blobURL = URL.createObjectURL(b);
const worker = new Worker(blobURL);
worker.onmessage = (): void => {
  Deno.exit(0);
};
