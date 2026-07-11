// Asserts the output of `deno sync-types` in global-cache mode.

function fail(msg: string): never {
  throw new Error(msg);
}

// Global-cache mode must not materialize a node_modules tree.
try {
  Deno.statSync("node_modules");
  fail("node_modules should not exist in global-cache mode");
} catch (e) {
  if (!(e instanceof Deno.errors.NotFound)) throw e;
}

// A per-package referenced project is generated for every resolved npm cache
// copy under `.deno/npm/<folder>/tsconfig.json`.
const projectsRoot = ".deno/npm";
const configs: string[] = [];
for (const entry of Deno.readDirSync(projectsRoot)) {
  if (!entry.isDirectory) continue;
  const cfg = `${projectsRoot}/${entry.name}/tsconfig.json`;
  Deno.statSync(cfg); // throws if the tsconfig is missing
  configs.push(cfg);
}
if (configs.length === 0) fail("no generated npm cache projects");

// The dependency edge `@denotest/dual-cjs-esm-dep -> @denotest/dual-cjs-esm`
// must appear in a generated project's `paths`, resolved through the
// dependency's package.json rather than a raw node_modules lookup.
let sawDependencyPath = false;
for (const cfg of configs) {
  const json = JSON.parse(Deno.readTextFileSync(cfg));
  const paths = json.compilerOptions?.paths ?? {};
  if ("@denotest/dual-cjs-esm" in paths) {
    sawDependencyPath = true;
  }
}
if (!sawDependencyPath) {
  fail("expected a generated project mapping @denotest/dual-cjs-esm in paths");
}

console.log("ok");
