export async function resolve(specifier, context, nextResolve) {
  if (specifier === "virtual:chain") {
    return { url: "file:///chain_module.js", shortCircuit: true };
  }
  return nextResolve(specifier, context);
}

export async function load(url, context, nextLoad) {
  if (url === "file:///chain_module.js") {
    return {
      source: 'export const value = "from chain-a";',
      format: "module",
      shortCircuit: true,
    };
  }
  return nextLoad(url, context);
}
