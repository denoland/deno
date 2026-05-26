import { registerHooks } from "node:module";

console.log("hook module loaded");

registerHooks({
  resolve(specifier, context, nextResolve) {
    console.log("hook:resolve", specifier);
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    console.log("hook:load", url);
    return nextLoad(url, context);
  },
});
