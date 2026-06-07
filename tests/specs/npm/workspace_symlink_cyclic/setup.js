// Recreate the `node_modules` layout that `npm install` produces for an npm
// workspace: each workspace package is symlinked into the root `node_modules`.
Deno.mkdirSync("node_modules", { recursive: true });
for (const name of ["pkg-a", "pkg-b", "main-project"]) {
  Deno.symlinkSync(`../packages/${name}`, `node_modules/${name}`);
}
