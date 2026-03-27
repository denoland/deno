// A resolve hook that redirects module resolution
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join } from "node:path";

export function resolve(specifier, context, nextResolve) {
  if (specifier === "my-virtual-module") {
    // Redirect to a real file
    const thisDir = dirname(fileURLToPath(context.parentURL));
    return {
      url: pathToFileURL(join(thisDir, "redirected.mjs")).href,
      shortCircuit: true,
    };
  }
  return nextResolve(specifier, context);
}
