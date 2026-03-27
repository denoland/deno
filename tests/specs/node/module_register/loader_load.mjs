// A load hook that transforms module source
export function load(url, context, nextLoad) {
  const result = nextLoad(url, context);
  if (url.endsWith("target.mjs")) {
    // Replace the module source with a transformed version
    return {
      format: "module",
      source: 'export const value = "transformed";',
      shortCircuit: true,
    };
  }
  return result;
}
