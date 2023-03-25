await import("./single_compile_with_reload_dyn.ts");
console.log("1");
await import("./single_compile_with_reload_dyn.ts");
console.log("2");
await new Promise((r) =>
  new Worker(
    import.meta.resolve("./single_compile_with_reload_worker.ts"),
    { type: "module" },
  ).onmessage = r
);
console.log("3");
await new Promise((r) =>
  new Worker(
    import.meta.resolve("./single_compile_with_reload_worker.ts"),
    { type: "module" },
  ).onmessage = r
);
console.log("4");
