export const osType = (() => {
  if (globalThis.Deno != null) {
    return Deno.build.os;
  }

  // deno-lint-ignore no-explicit-any
  const navigator = (globalThis as any).navigator;
  if (navigator?.appVersion?.includes?.("Win") ?? false) {
    return "windows";
  }

  return "linux";
})();

export const isWindows = osType === "windows";
