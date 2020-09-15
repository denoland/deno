const { diagnostics, emitMap } = await Deno.compile(
  "main.ts",
  {
    "main.ts": `document.getElementById("foo");`,
  },
  {
    lib: ["dom", "esnext"],
  },
);

console.log(diagnostics);
console.log(Object.keys(emitMap));
