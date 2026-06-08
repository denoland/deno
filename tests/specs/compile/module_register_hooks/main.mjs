import { registerHooks } from "node:module";
import { pathToFileURL } from "node:url";
import { join } from "node:path";
import assert from "node:assert/strict";

const cwdUrl = pathToFileURL(Deno.cwd() + "/").href;
const resolvedExternalUrl = pathToFileURL(
  join(Deno.cwd(), "external_resolved.mjs"),
).href;
const loadedExternalUrl = pathToFileURL(
  join(Deno.cwd(), "external_loaded.mjs"),
).href;

const calls = [];
const hook = registerHooks({
  resolve(specifier, context, nextResolve) {
    calls.push(`resolve:${specifier}`);
    assert.ok(Array.isArray(context.conditions));

    if (specifier === "external:resolved") {
      assert.equal(context.parentURL, import.meta.url);
      return { url: resolvedExternalUrl, shortCircuit: true };
    }

    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    calls.push(`load:${url}`);
    assert.ok(context.importAttributes);

    if (url === loadedExternalUrl) {
      return {
        format: "module",
        shortCircuit: true,
        source: 'export const value = "from load hook";',
      };
    }

    return nextLoad(url, context);
  },
});

const resolved = await import("external:resolved");
assert.equal(resolved.value, "from external file");

const loaded = await import(new URL("external_loaded.mjs", cwdUrl).href);
assert.equal(loaded.value, "from load hook");

hook.deregister();

assert.ok(calls.includes("resolve:external:resolved"));
assert.ok(calls.includes(`load:${loadedExternalUrl}`));

console.log("compile registerHooks works");
