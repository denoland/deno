export async function resolve(specifier, context, nextResolve) {
  if (specifier === "virtual:order-test") {
    // Async hook: resolve to a different URL
    return { url: "file:///order_async.js", shortCircuit: true };
  }
  return nextResolve(specifier, context);
}

export async function load(url, context, nextLoad) {
  if (url === "file:///order_async.js") {
    return {
      source: 'export const source = "async";',
      format: "module",
      shortCircuit: true,
    };
  }
  return nextLoad(url, context);
}
