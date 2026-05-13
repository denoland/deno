import { registerHooks } from "node:module";

// Custom loader that transforms .yaml files into JSON modules.
// This exercises three fixes:
// 1. Default nextResolve must return the resolved URL (not null)
//    so the resolve hook can inspect the file extension.
// 2. The format from resolve hooks must flow to load hooks as
//    context.format.
// 3. The Rust module loader must respect the "json" format from
//    load hooks instead of hardcoding JavaScript.
const hook = registerHooks({
  resolve(specifier, context, nextResolve) {
    const result = nextResolve(specifier, context);
    if (result.url && result.url.endsWith(".yaml")) {
      return { ...result, format: "yaml" };
    }
    return result;
  },
  load(url, context, nextLoad) {
    if (context.format !== "yaml") {
      return nextLoad(url);
    }
    const result = nextLoad(url, { format: "module" });
    // Parse YAML (simple key: value format for this test)
    const source = result.source;
    const obj = {};
    for (const line of source.split("\n")) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      const colon = trimmed.indexOf(":");
      if (colon === -1) continue;
      const key = trimmed.slice(0, colon).trim();
      const val = trimmed.slice(colon + 1).trim();
      obj[key] = val;
    }
    return {
      format: "json",
      source: JSON.stringify(obj),
      shortCircuit: true,
    };
  },
});

const mod = await import("./test_data.yaml");
console.log("greeting:", mod.default.greeting);
console.log("name:", mod.default.name);

hook.deregister();
console.log("done");
