// Regression test for https://github.com/denoland/deno/issues/35163
// A resolve-only hook rewriting specifiers (e.g. appending a fragment for
// cache busting) must work for nested imports too: the rewritten URLs are
// never seen by the initial graph preparation pass.
import module from "node:module";

let gen = 0;
module.registerHooks({
  resolve(specifier, context, nextResolve) {
    return nextResolve(`${specifier}#${gen}`, { ...context });
  },
});

const a = await import("./src/data.ts");
console.log("first:", a.title);

// Bumping the generation re-resolves every module in the chain to fresh
// URLs, forcing re-evaluation (HMR-style cache busting).
gen++;
const b = await import("./src/data.ts");
console.log("second:", b.title);
console.log("distinct module instances:", a !== b);
