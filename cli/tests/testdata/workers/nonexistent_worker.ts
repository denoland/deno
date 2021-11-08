const w = new Worker(new URL("doesnt_exist.js", import.meta.url).href, {
  type: "module",
});

w.postMessage("hello");
