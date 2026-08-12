// Read from the built `dist/` directory to prove it was bundled into
// the compiled binary's VFS. Use import.meta.dirname so the path
// resolves against the VFS rather than the runtime cwd.
const html = await Deno.readTextFile(
  `${import.meta.dirname}/dist/index.html`,
);
console.log("vite ssr server started");
console.log(html.trim());
