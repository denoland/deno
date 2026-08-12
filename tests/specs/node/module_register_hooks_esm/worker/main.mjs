const worker = new Worker(import.meta.resolve("./worker.mjs"), {
  type: "module",
});

worker.onmessage = (e) => {
  console.log("from worker:", e.data);
  worker.terminate();
};
