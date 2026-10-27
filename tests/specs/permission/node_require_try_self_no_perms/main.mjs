// Regression test: op_require_try_self walks up from a caller-controlled
// parent path reading package.json files, so it must require read permission
// for that path, like its sibling op_require_package_imports_resolve. Here the
// require's parent path points inside ./selfpkg, whose package.json declares a
// self-referential "exports". Without --allow-read, resolving the package's
// own name must throw NotCapable rather than silently read package.json off
// disk.

import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

// Use fileURLToPath rather than URL.pathname: on Windows `.pathname` yields a
// leading-slash path like `/C:/...` that isn't a valid OS path, which makes
// createRequire fail before it reaches op_require_try_self.
const parent = fileURLToPath(new URL("./selfpkg/sub/x.js", import.meta.url));
const require = createRequire(parent);

try {
  require.resolve("selfpkg");
  console.log("result: resolved");
} catch (e) {
  console.log(
    "result:",
    e.name === "NotCapable" ? "denied" : `other:${e.name}`,
  );
}
