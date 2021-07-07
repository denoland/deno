const worker = new Worker(new URL("./worker.js", import.meta.url), {
  type: "module",
});

const sab = new SharedArrayBuffer(1);
console.log(new Uint8Array(sab));

setInterval(() => {
  console.log(new Uint8Array(sab));
}, 100);

worker.onmessage = () => {
  worker.postMessage(sab);
};
