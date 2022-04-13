import { dirname, fromFileUrl, join } from "../../test_util/std/path/mod.ts";
import { expandGlobSync } from "../../test_util/std/fs/mod.ts";

const ROOT_DIR = join(dirname(fromFileUrl(import.meta.url)), "..", "..");

export function rebootstrap(exts) {
  [
    "core/00_primordials.js",
    ...exts.map((e) => `ext/${e}/*.js`),
  ]
    .map((pattern) => join(ROOT_DIR, pattern))
    .map((pattern) => [...expandGlobSync(pattern)])
    .flat()
    .map((entry) => entry.path)
    .forEach((file) => {
      Deno.core.evalContext(Deno.readTextFileSync(file), file);
    });
  const bootstrap = globalThis.__bootstrap;
  delete globalThis.__bootstrap;
  // Patch dispatchEvent so we don't crash when MainWorker exits via:
  // `window.dispatchEvent(new Event('unload'))`
  // which fails since symbols are mangled during rebootstrap
  globalThis.dispatchEvent = () => {};
  return bootstrap;
}
