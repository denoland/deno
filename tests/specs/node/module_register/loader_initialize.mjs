// A loader that uses the initialize hook to receive data
let initData;

export function initialize(data) {
  initData = data;
}

export function load(url, context, nextLoad) {
  if (url.endsWith("target.mjs")) {
    return {
      format: "module",
      source: `export const value = ${
        JSON.stringify(initData?.greeting ?? "no data")
      };`,
      shortCircuit: true,
    };
  }
  return nextLoad(url, context);
}
