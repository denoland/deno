// Regression test for https://github.com/denoland/deno/issues/35372 —
// `vm.SourceTextModule` with `import.meta` used to panic at
// `host_initialize_import_meta_object_callback` (libs/core/runtime/bindings.rs)
// because vm-created v8::Modules aren't registered in deno_core's module map.
// The runtime now falls back to a hook registered by `node:vm`, which invokes
// the user-supplied `initializeImportMeta` callback (if any). Like Node's
// `vm.SourceTextModule`, `import.meta` is otherwise left empty — `url` is
// never auto-populated.

import assert from "node:assert";
import vm from "node:vm";

// 1. `initializeImportMeta` is invoked and can set arbitrary properties.
// Only the callback's own props are present — `url` is not auto-populated.
{
  const context = vm.createContext({});
  const mod = new vm.SourceTextModule(
    "export const captured = { hasUrl: 'url' in import.meta, url: import.meta.url, prop: import.meta.prop };",
    {
      identifier: "vm:test-import-meta",
      context,
      initializeImportMeta(meta) {
        meta.prop = { extra: true };
      },
    },
  );
  mod.linkRequests([]);
  mod.instantiate();
  await mod.evaluate();
  assert.strictEqual(mod.namespace.captured.hasUrl, false);
  assert.strictEqual(mod.namespace.captured.url, undefined);
  assert.deepStrictEqual(mod.namespace.captured.prop, { extra: true });
  console.log("ok initializeImportMeta");
}

// 2. With no `initializeImportMeta` callback, `import.meta` is completely
// empty (matches Node — it does not auto-populate `url`/`main`/`resolve`).
{
  const context = vm.createContext({});
  // Compute `keyCount` inside the module — `import.meta`'s keys array lives
  // in the vm context's realm, so comparing it directly to a main-realm `[]`
  // would trip `deepStrictEqual`'s prototype check.
  const mod = new vm.SourceTextModule(
    "export const keyCount = Object.keys(import.meta).length; export const url = import.meta.url;",
    { context },
  );
  mod.linkRequests([]);
  mod.instantiate();
  await mod.evaluate();
  assert.strictEqual(mod.namespace.keyCount, 0);
  assert.strictEqual(mod.namespace.url, undefined);
  console.log("ok import.meta empty");
}

// 3. Supplying `identifier` does NOT seed `import.meta.url` — Node leaves
// `import.meta` entirely under the user's control.
{
  const context = vm.createContext({});
  const mod = new vm.SourceTextModule(
    "export const meta = { hasUrl: 'url' in import.meta, url: import.meta.url };",
    { context, identifier: "vm:test-no-init" },
  );
  mod.linkRequests([]);
  mod.instantiate();
  await mod.evaluate();
  assert.strictEqual(mod.namespace.meta.hasUrl, false);
  assert.strictEqual(mod.namespace.meta.url, undefined);
  console.log("ok no callback");
}

// 4. Original repro from issue #35372 — must not panic. The module body
// throws a ReferenceError (the module isn't bound to the contextified
// context, so `secret` is undefined), but the runtime stays alive.
{
  vm.createContext({ secret: 42 });
  const mod = new vm.SourceTextModule(
    "Object.getPrototypeOf(import.meta.prop).secret = secret;",
    {
      initializeImportMeta(meta) {
        meta.prop = {};
      },
    },
  );
  mod.linkRequests([]);
  mod.instantiate();
  await assert.rejects(mod.evaluate(), ReferenceError);
  console.log("ok original repro");
}
