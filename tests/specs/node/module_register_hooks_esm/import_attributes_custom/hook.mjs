import { registerHooks } from "node:module";

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier.endsWith("mod.css")) {
      console.log(
        "hook:resolve",
        specifier,
        JSON.stringify(context.importAttributes),
      );
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url.endsWith("mod.css")) {
      console.log(
        "hook:load",
        url.split("/").pop(),
        JSON.stringify(context.importAttributes),
      );
      return {
        source: 'export default "css-content";',
        format: "module",
        shortCircuit: true,
      };
    }
    return nextLoad(url, context);
  },
});
