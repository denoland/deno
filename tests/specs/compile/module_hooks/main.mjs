import { registerHooks } from "node:module";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

// Test that registerHooks load hooks work in compiled binaries.
// The hook transforms the source of hello.js before it's executed.
const hook = registerHooks({
  load(url, context, nextLoad) {
    if (url.endsWith("hello.js")) {
      return {
        source: 'export const greeting = "transformed by hook";',
        format: "module",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});

// hello.js is an external file (not embedded) so hooks can intercept it.
const helloUrl = pathToFileURL(join(process.cwd(), "hello.js")).href;
const mod = await import(helloUrl);
console.log(mod.greeting);

hook.deregister();
