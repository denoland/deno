// Asserts the output of `deno sync-types` in global-cache mode.

function fail(msg: string): never {
  throw new Error(msg);
}

// Every generated `paths` target must resolve to a real file/dir on disk, so a
// mis-resolved mapping (e.g. an `exports` subpath pointing at a nonexistent
// declaration) is caught without needing to run tsc.
function assertPathsResolve(
  cfgPath: string,
  paths: Record<string, string[]>,
) {
  for (const [key, targets] of Object.entries(paths)) {
    for (const target of targets) {
      // Wildcard mappings (`.../*`, `.../*.d.ts`) resolve a subpath under the
      // prefix before the `*`; assert that prefix directory exists.
      const star = target.indexOf("*");
      const probe = star === -1 ? target : target.slice(0, star);
      try {
        Deno.statSync(probe);
      } catch (e) {
        if (e instanceof Deno.errors.NotFound) {
          fail(
            `${cfgPath}: paths[${JSON.stringify(key)}] -> ${target} missing`,
          );
        }
        throw e;
      }
    }
  }
}

// Global-cache mode must not materialize a node_modules tree.
try {
  Deno.statSync("node_modules");
  fail("node_modules should not exist in global-cache mode");
} catch (e) {
  if (!(e instanceof Deno.errors.NotFound)) throw e;
}

// The root config's own `paths` (jsr:/npm:/http mappings) must resolve too.
{
  const root = JSON.parse(Deno.readTextFileSync(".deno/tsconfig.json"));
  assertPathsResolve(".deno/tsconfig.json", root.compilerOptions?.paths ?? {});
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
// dependency's package.json rather than a raw node_modules lookup, and every
// generated project's paths + declaration files must exist on disk.
let sawDependencyPath = false;
for (const cfg of configs) {
  const json = JSON.parse(Deno.readTextFileSync(cfg));
  const paths = json.compilerOptions?.paths ?? {};
  if ("@denotest/dual-cjs-esm" in paths) {
    sawDependencyPath = true;
  }
  assertPathsResolve(cfg, paths);
  // The project's declaration `files` must exist as well.
  for (const file of json.files ?? []) {
    try {
      Deno.statSync(file);
    } catch (e) {
      if (e instanceof Deno.errors.NotFound) {
        fail(`${cfg}: files entry ${file} missing`);
      }
      throw e;
    }
  }
}
if (!sawDependencyPath) {
  fail("expected a generated project mapping @denotest/dual-cjs-esm in paths");
}

console.log("ok");
