// Copyright 2018-2026 the Deno authors. MIT license.

let expectedDepth: number | null = 4;

const customInspect: Deno.CustomInspect = (inspect, options) => {
  const depth: number | null = options.depth;
  const indentLevel: number = options.indentLevel;
  const forwardedOptions: Deno.InspectOptions = options;
  if (depth !== expectedDepth || indentLevel !== 0) {
    throw new Error("Unexpected custom inspection options");
  }
  return `Wrapper ${inspect({ value: 42 }, forwardedOptions)}`;
};

const value = { [Symbol.for("Deno.customInspect")]: customInspect };
if (Deno.inspect(value) !== "Wrapper { value: 42 }") {
  throw new Error("Custom inspection did not forward the options");
}

expectedDepth = null;
if (Deno.inspect(value, { depth: null }) !== "Wrapper { value: 42 }") {
  throw new Error("Custom inspection did not preserve unlimited depth");
}
