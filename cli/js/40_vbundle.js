// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check

/**
 * Vbundle plugin runtime for Deno.
 *
 * This module provides the JavaScript runtime for vbundle plugins.
 * Plugins follow a Vite/Rollup-like API with resolveId, load, and transform hooks.
 */

import { core, internals } from "ext:core/mod.js";

const { op_vbundle_emit_file } = core.ops;

/**
 * @typedef {Object} PluginInfo
 * @property {string} name
 * @property {string[]} extensions
 * @property {boolean} hasResolve
 * @property {boolean} hasLoad
 * @property {boolean} hasTransform
 */

/**
 * @typedef {Object} ResolveResult
 * @property {string} id
 * @property {boolean} [external]
 * @property {boolean} [sideEffects]
 */

/**
 * @typedef {Object} LoadResult
 * @property {string} code
 * @property {string} [map]
 * @property {string} [loader]
 * @property {boolean} [sideEffects]
 */

/**
 * @typedef {Object} TransformResult
 * @property {string} code
 * @property {string} [map]
 */

/**
 * @typedef {Object} ResolveOptions
 * @property {boolean} [isEntry]
 * @property {string} [kind]
 */

/**
 * @typedef {Object} Plugin
 * @property {string} name
 * @property {string[]} [extensions]
 * @property {(source: string, importer: string | null, options: ResolveOptions) => ResolveResult | null | undefined | Promise<ResolveResult | null | undefined>} [resolveId]
 * @property {(id: string) => LoadResult | null | undefined | Promise<LoadResult | null | undefined>} [load]
 * @property {(code: string, id: string) => TransformResult | null | undefined | Promise<TransformResult | null | undefined>} [transform]
 */

/** @type {Plugin[]} */
const plugins = [];

/** @type {Set<string>} */
const installedPluginNames = new Set();

/**
 * Install plugins from their default exports.
 * @param {Plugin[]} pluginExports
 * @returns {PluginInfo[]}
 */
export function installPlugins(pluginExports) {
  /** @type {PluginInfo[]} */
  const infos = [];

  for (const plugin of pluginExports) {
    const info = installPlugin(plugin);
    infos.push(info);
  }

  return infos;
}

/**
 * Install a single plugin.
 * @param {Plugin} plugin
 * @returns {PluginInfo}
 */
function installPlugin(plugin) {
  if (typeof plugin !== "object" || plugin === null) {
    throw new Error("Vbundle plugin must be an object");
  }

  if (typeof plugin.name !== "string" || plugin.name.length === 0) {
    throw new Error("Vbundle plugin must have a 'name' string property");
  }

  if (installedPluginNames.has(plugin.name)) {
    throw new Error(`Vbundle plugin '${plugin.name}' is already installed`);
  }

  plugins.push(plugin);
  installedPluginNames.add(plugin.name);

  return {
    name: plugin.name,
    extensions: plugin.extensions ?? [],
    hasResolve: typeof plugin.resolveId === "function",
    hasLoad: typeof plugin.load === "function",
    hasTransform: typeof plugin.transform === "function",
  };
}

/**
 * Call resolveId hooks on all plugins.
 * Returns the first non-null result.
 *
 * @param {string} source - The import specifier to resolve
 * @param {string | null} importer - The module that is importing
 * @param {ResolveOptions} options - Resolution options
 * @returns {ResolveResult | null}
 */
export function resolveId(source, importer, options) {
  for (const plugin of plugins) {
    if (typeof plugin.resolveId !== "function") {
      continue;
    }

    try {
      const result = plugin.resolveId(source, importer, options);

      // Handle promises synchronously for now
      // TODO: Support async plugins
      if (result && typeof result === "object" && "then" in result) {
        throw new Error(
          `Async resolveId hooks are not yet supported (plugin: ${plugin.name})`
        );
      }

      if (result != null) {
        // Normalize result
        if (typeof result === "string") {
          return { id: result };
        }
        return result;
      }
    } catch (err) {
      throw new Error(`Plugin '${plugin.name}' resolveId hook failed`, {
        cause: err,
      });
    }
  }

  return null;
}

/**
 * Call load hooks on all plugins.
 * Returns the first non-null result.
 *
 * @param {string} id - The resolved module id
 * @returns {LoadResult | null}
 */
export function load(id) {
  for (const plugin of plugins) {
    if (typeof plugin.load !== "function") {
      continue;
    }

    try {
      const result = plugin.load(id);

      // Handle promises synchronously for now
      if (result && typeof result === "object" && "then" in result) {
        throw new Error(
          `Async load hooks are not yet supported (plugin: ${plugin.name})`
        );
      }

      if (result != null) {
        // Normalize result
        if (typeof result === "string") {
          return { code: result };
        }
        return result;
      }
    } catch (err) {
      throw new Error(`Plugin '${plugin.name}' load hook failed for '${id}'`, {
        cause: err,
      });
    }
  }

  return null;
}

/**
 * Call transform hooks on all plugins.
 * Each plugin can transform the code, and transformations are chained.
 *
 * @param {string} id - The module id
 * @param {string} code - The source code to transform
 * @returns {TransformResult | null}
 */
export function transform(id, code) {
  let currentCode = code;
  let combinedMap = null;
  let hasTransformed = false;

  for (const plugin of plugins) {
    if (typeof plugin.transform !== "function") {
      continue;
    }

    try {
      const result = plugin.transform(currentCode, id);

      // Handle promises synchronously for now
      if (result && typeof result === "object" && "then" in result) {
        throw new Error(
          `Async transform hooks are not yet supported (plugin: ${plugin.name})`
        );
      }

      if (result != null) {
        hasTransformed = true;

        // Normalize result
        if (typeof result === "string") {
          currentCode = result;
        } else {
          currentCode = result.code;
          // TODO: Combine source maps
          if (result.map) {
            combinedMap = result.map;
          }
        }
      }
    } catch (err) {
      throw new Error(
        `Plugin '${plugin.name}' transform hook failed for '${id}'`,
        { cause: err }
      );
    }
  }

  if (!hasTransformed) {
    return null;
  }

  return {
    code: currentCode,
    map: combinedMap ?? undefined,
  };
}

/**
 * Emit a file from a plugin.
 * @param {Object} options
 * @param {string} options.type - 'chunk' or 'asset'
 * @param {string} [options.fileName]
 * @param {string} [options.name]
 * @param {string} [options.source]
 * @returns {string} Reference ID for the emitted file
 */
export function emitFile(options) {
  return op_vbundle_emit_file(options);
}

/**
 * Plugin context passed to hooks.
 */
export class PluginContext {
  /**
   * Emit a file (chunk or asset).
   * @param {Object} options
   * @returns {string}
   */
  emitFile(options) {
    return emitFile(options);
  }

  /**
   * Log a warning.
   * @param {string} message
   */
  warn(message) {
    console.warn(`[vbundle] ${message}`);
  }

  /**
   * Log an error.
   * @param {string} message
   */
  error(message) {
    throw new Error(message);
  }
}

// Export functions to Deno internals for Rust to call
internals.installPlugins = installPlugins;
internals.resolveId = resolveId;
internals.load = load;
internals.transform = transform;

// Also expose on Deno namespace for plugin authors
if (!Deno.vbundle) {
  Deno.vbundle = {};
}

Deno.vbundle.emitFile = emitFile;
Deno.vbundle.PluginContext = PluginContext;
