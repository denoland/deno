// Hook module for register() that transforms .yaml files into JSON modules.
// This exercises the worker-thread architecture: the hook module loads
// in the worker thread, and resolve/load hooks are called via message passing.

export async function resolve(specifier, context, nextResolve) {
  const result = await nextResolve(specifier, context);
  if (result.url && result.url.endsWith(".yaml")) {
    return { ...result, format: "yaml" };
  }
  return result;
}

export async function load(url, context, nextLoad) {
  if (context.format !== "yaml") {
    return nextLoad(url);
  }
  const result = await nextLoad(url, { format: "module" });
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
}
