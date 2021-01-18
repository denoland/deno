const { diagnostics, files } = await Deno.emit(
  "/main.ts",
  {
    sources: {
      "/main.ts":
        `/// <reference lib="dom" />\n\ndocument.getElementById("foo");\nDeno.args;`,
    },
    compilerOptions: {
      target: "es2018",
      lib: ["es2018", "deno.ns"],
    },
  },
);

console.log(diagnostics);
console.log(Object.keys(files).sort());
