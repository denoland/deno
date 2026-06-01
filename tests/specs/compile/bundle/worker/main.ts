const w = new Worker(new URL("./worker.ts", import.meta.url), {
  type: "module",
});
w.onmessage = (e) => {
  console.log("main got:", e.data);
  w.terminate();
};
w.postMessage("ping");
