// Regression test for https://github.com/denoland/deno/issues/35372 —
// `vm.SourceTextModule` with `import.meta` used to panic at
// `host_initialize_import_meta_object_callback` (libs/core/runtime/bindings.rs)
// because vm-created v8::Modules aren't registered in deno_core's module map.
// The runtime now falls back to a hook registered by `node:vm`, which sets
// `import.meta.url` from the module's `identifier` option and invokes the
// user-supplied `initializeImportMeta` callback.

import assert from "node:assert";
import vm from "node:vm";

// 1. `initializeImportMeta` is invoked and can set arbitrary properties.
{
  const context = vm.createContext({});
  const mod = new vm.SourceTextModule(
    "export const captured = { url: import.meta.url, prop: import.meta.prop };",
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
  assert.strictEqual(mod.namespace.captured.url, "vm:test-import-meta");
  assert.deepStrictEqual(mod.namespace.captured.prop, { extra: true });
  console.log("ok initializeImportMeta");
}

// 2. With no explicit `identifier`, `import.meta.url` falls back to the
// `vm:module(N)` default V8 calls the host-initialize-import-meta callback
// with.
{
  const context = vm.createContext({});
  const mod = new vm.SourceTextModule(
    "export const url = import.meta.url;",
    { context },
  );
  mod.linkRequests([]);
  mod.instantiate();
  await mod.evaluate();
  assert.match(mod.namespace.url, /^vm:module\(\d+\)$/);
  console.log("ok import.meta.url default");
}

// 3. Without an `initializeImportMeta` callback, `import.meta` still has
// `url` set but no user-defined props.
{
  const context = vm.createContext({});
  const mod = new vm.SourceTextModule(
    "export const meta = { url: import.meta.url, hasProp: 'prop' in import.meta };",
    { context, identifier: "vm:test-no-init" },
  );
  mod.linkRequests([]);
  mod.instantiate();
  await mod.evaluate();
  assert.strictEqual(mod.namespace.meta.url, "vm:test-no-init");
  assert.strictEqual(mod.namespace.meta.hasProp, false);
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
