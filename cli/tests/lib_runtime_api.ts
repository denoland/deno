const [errors, program] = await Deno.compile(
  "/main.ts",
  {
    "/main.ts": `document.getElementById("foo");`,
  },
  {
    lib: ["dom", "esnext"],
  },
);

console.log(errors);
console.log(Object.keys(program).sort());
