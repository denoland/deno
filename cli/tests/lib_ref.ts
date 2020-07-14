const [errors, program] = await Deno.compile(
  "main.ts",
  {
    "main.ts":
      `/// <reference lib="dom" />\n\ndocument.getElementById("foo");\nDeno.args;`,
  },
  {
    target: "es2018",
    lib: ["es2018", "deno.ns"],
  },
);

console.log(errors);
console.log(Object.keys(program));
