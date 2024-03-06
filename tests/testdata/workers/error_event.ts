const worker = new Worker(import.meta.resolve("./error.ts"), {
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
