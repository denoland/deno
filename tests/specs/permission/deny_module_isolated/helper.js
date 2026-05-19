// Loaded into a sibling v8::Context via --deny-module-isolated.
// The realm has no `Deno` global, so any direct Deno access throws.
export function describeEnv() {
  const hasDeno = typeof globalThis.Deno !== "undefined";
  const denoIsObject = typeof Deno !== "undefined";
  return { hasDeno, denoIsObject };
}

export function tryReadSelf() {
  try {
    Deno.statSync(new URL(import.meta.url));
    return "succeeded";
  } catch (err) {
    return `${err.name}: ${err.message}`;
  }
}
