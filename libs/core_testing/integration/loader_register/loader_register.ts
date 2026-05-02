// Copyright 2018-2026 the Deno authors. MIT license.
import { register } from "checkin:loader";

// Register hooks that intercept virtual: specifiers
register({
  async resolve(specifier, context, nextResolve) {
    if (specifier === "virtual:greet") {
      return { url: "test:///integration/loader_register/virtual_greet.js" };
    }
    if (specifier === "virtual:real-module") {
      return { url: "test:///integration/loader_register/real_module.ts" };
    }
    if (specifier === "virtual:async-source") {
      return { url: "test:///integration/loader_register/virtual_async.js" };
    }
    // Delegate to default resolution
    return nextResolve(specifier, context);
  },
  async load(url, _context, nextLoad) {
    if (url === "test:///integration/loader_register/virtual_greet.js") {
      return {
        source:
          `export function greet(name) { return "Hello, " + name + "!"; }`,
      };
    }
    if (url === "test:///integration/loader_register/virtual_async.js") {
      // Simulate async work
      await new Promise((resolve) => setTimeout(resolve, 1));
      return { source: `export const message = "loaded via promise";` };
    }
    // Delegate to default loading (e.g. for real_module.ts on disk)
    return nextLoad(url);
  },
});

// Dynamic imports trigger the async resolve + load paths
const { greet } = await import("virtual:greet");
console.log(greet("world"));

const { value } = await import("virtual:real-module");
console.log(value);

const { message } = await import("virtual:async-source");
console.log(message);
