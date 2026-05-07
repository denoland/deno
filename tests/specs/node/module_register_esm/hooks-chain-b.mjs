// This hook is registered AFTER chain-a, so it runs FIRST (LIFO).
// It intercepts "virtual:chain" with a different URL, so chain-a never sees it.
export async function resolve(specifier, context, nextResolve) {
  if (specifier === "virtual:chain") {
    return { url: "file:///chain_b_module.js", shortCircuit: true };
  }
  return nextResolve(specifier, context);
}

export async function load(url, context, nextLoad) {
  if (url === "file:///chain_b_module.js") {
    return {
      source: 'export const value = "from chain-b (last registered wins)";',
      format: "module",
      shortCircuit: true,
    };
  }
  return nextLoad(url, context);
}
