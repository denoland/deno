// Read from `dist/` to prove the include path was rebased against the
// detected app directory (./myapp) rather than the caller's cwd.
const html = await Deno.readTextFile(
  `${import.meta.dirname}/dist/index.html`,
);
console.log("explicit dir server started");
console.log(html.trim());
