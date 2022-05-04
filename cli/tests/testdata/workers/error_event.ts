const worker = new Worker(new URL("error.ts", import.meta.url).href, {
  type: "module",
});
worker.addEventListener("error", (e) => {
  console.log({
    "message": e.message,
    "filename": e.filename?.slice?.(-100),
    "lineno": e.lineno,
    "colno": e.colno,
  });
});
