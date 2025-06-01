declare global {
  const polyfill: () => void;
}

// deno-lint-ignore no-explicit-any
(globalThis as any).polyfill = () => {
  console.log("polyfill");
};
