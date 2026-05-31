// Regression test for https://github.com/denoland/deno/issues/31456
// A wasm file included via `--include` has a host import
// ("some_import"."dummy") that is provided explicitly at instantiation, not
// through the module graph. `deno compile` must embed it as an asset rather
// than treating it as a module graph root and failing to resolve that import.
// See #27505.
//
// The tiny.wasm reproducer was contributed by @CertainLach in #31457.
const bytes = await Deno.readFile(new URL("./tiny.wasm", import.meta.url));
const { instance } = await WebAssembly.instantiate(bytes, {
  some_import: { dummy: () => console.log("dummy called") },
});
(instance.exports.run as () => void)();
