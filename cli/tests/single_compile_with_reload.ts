await import("./single_compile_with_reload_dyn.ts");
console.log("1");
await import("./single_compile_with_reload_dyn.ts");
console.log("2");
await new Promise((r) =>
  new Worker(
    new URL("single_compile_with_reload_worker.ts", import.meta.url).href,
    { type: "module" },
  ).onmessage = r
);
console.log("3");
await new Promise((r) =>
  new Worker(
    new URL("single_compile_with_reload_worker.ts", import.meta.url).href,
    { type: "module" },
  ).onmessage = r
);
console.log("4");
