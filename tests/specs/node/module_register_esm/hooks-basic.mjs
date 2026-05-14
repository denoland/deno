export async function resolve(specifier, context, nextResolve) {
  if (specifier === "virtual:hello") {
    return { url: "file:///virtual_hello.js", shortCircuit: true };
  }
  return nextResolve(specifier, context);
}

export async function load(url, context, nextLoad) {
  if (url === "file:///virtual_hello.js") {
    return {
      source: 'export const greeting = "hello from register hooks";',
      format: "module",
      shortCircuit: true,
    };
  }
  return nextLoad(url, context);
}
