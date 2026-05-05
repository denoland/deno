let thePort = null;

export async function initialize(data) {
  thePort = data.port;
  thePort.postMessage("initialize");
}

export async function resolve(specifier, context, nextResolve) {
  if (specifier === "virtual:tracked") {
    thePort.postMessage("resolve virtual:tracked");
    return { url: "file:///virtual_tracked.js", shortCircuit: true };
  }
  return nextResolve(specifier, context);
}

export async function load(url, context, nextLoad) {
  if (url === "file:///virtual_tracked.js") {
    thePort.postMessage("load virtual:tracked");
    return {
      source: 'export const value = "tracked";',
      format: "module",
      shortCircuit: true,
    };
  }
  return nextLoad(url, context);
}
