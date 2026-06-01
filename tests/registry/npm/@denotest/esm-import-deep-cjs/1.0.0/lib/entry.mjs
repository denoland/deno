// Mirrors the jiti layout: an ESM entry in `lib/` that default-imports a
// deep CJS file in a sibling `dist/` directory, and that CJS file in turn
// require()s another CJS module. The whole transitive CJS graph has to be
// pulled in. Under plain `deno compile` factory config (code translator in
// ModuleLoader mode) these deep CJS files were skipped from the module
// graph and esbuild's load handler returned nothing for them, tripping a
// parse error. `compile --bundle` forces bundle-style resolution so they
// are included.
import factory from "../dist/bundled.cjs";

export default function () {
  return factory();
}
