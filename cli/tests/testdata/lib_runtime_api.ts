const { diagnostics, files } = await Deno.emit(
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
console.log(Object.keys(files).sort());
