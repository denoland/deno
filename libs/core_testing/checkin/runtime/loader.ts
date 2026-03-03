// Copyright 2018-2026 the Deno authors. MIT license.

const core = Deno.core;

/**
 * Register a resolve mapping. When the module loader encounters `specifier`,
 * it will resolve it to `resolved` using the async resolution path.
 */
export function registerResolveMapping(
  specifier: string,
  resolved: string,
): void {
  core.ops.op_loader_register_resolve(specifier, resolved);
}
