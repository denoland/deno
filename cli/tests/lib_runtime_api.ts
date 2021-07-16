const { diagnostics, modules } = await Deno.emit(
  "/main.ts",
  {
    sources: {
      "/main.ts": `document.getElementById("foo");`,
    },
    compilerOptions: {
      lib: ["dom", "esnext"],
    },
  },
);

console.log(diagnostics);
console.log(modules);
