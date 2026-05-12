import { register } from "node:module";

// Registering a passthrough hook flips on the global "resolve_active" flag
// so resolves are routed through the hooks-worker bridge.
//
// `@denotest/dual-cjs-esm-dep`'s ESM entry does
//   import { getKind } from "@denotest/dual-cjs-esm";
// i.e. a bare intra-package dependency import. That resolution needs the
// dep package's `package.json` for context, which the resolver can only
// find if the referrer is the canonical file URL of the parent module --
// not the original `npm:` request specifier.
register("../hooks-passthrough.mjs", import.meta.url);

const { getKind } = await import("npm:@denotest/dual-cjs-esm-dep");
console.log("kind:", getKind());
