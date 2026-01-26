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
 * @property {boolean} hasBuildStart
 * @property {boolean} hasBuildEnd
 * @property {boolean} hasRenderChunk
 * @property {boolean} hasGenerateBundle
 * @property {'pre' | 'post' | undefined} enforce
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
 * @typedef {Object} RenderChunkResult
 * @property {string} code
 * @property {string} [map]
 */

/**
 * @typedef {Object} ResolveOptions
 * @property {boolean} [isEntry]
 * @property {string} [kind]
 */

/**
 * @typedef {Object} ChunkInfo
 * @property {string} fileName
 * @property {string} name
 * @property {string[]} modules
 * @property {boolean} isEntry
 * @property {boolean} isDynamicEntry
 */

/**
 * @typedef {Object} Plugin
 * @property {string} name
 * @property {string[]} [extensions]
 * @property {'pre' | 'post'} [enforce]
 * @property {() => void | Promise<void>} [buildStart]
 * @property {() => void | Promise<void>} [buildEnd]
 * @property {(source: string, importer: string | null, options: ResolveOptions) => ResolveResult | string | null | undefined | Promise<ResolveResult | string | null | undefined>} [resolveId]
 * @property {(id: string) => LoadResult | string | null | undefined | Promise<LoadResult | string | null | undefined>} [load]
 * @property {(code: string, id: string) => TransformResult | string | null | undefined | Promise<TransformResult | string | null | undefined>} [transform]
 * @property {(code: string, chunk: ChunkInfo) => RenderChunkResult | string | null | undefined | Promise<RenderChunkResult | string | null | undefined>} [renderChunk]
 * @property {(bundle: Record<string, ChunkInfo>) => void | Promise<void>} [generateBundle]
 */

/** @type {Plugin[]} */
const plugins = [];

/** @type {Plugin[]} */
let sortedPlugins = [];

/** @type {Set<string>} */
const installedPluginNames = new Set();

/**
 * Sort plugins by enforce order: pre -> normal -> post
 */
function sortPlugins() {
  const pre = plugins.filter((p) => p.enforce === "pre");
  const normal = plugins.filter((p) => !p.enforce);
  const post = plugins.filter((p) => p.enforce === "post");
  sortedPlugins = [...pre, ...normal, ...post];
}

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

  sortPlugins();

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

  // Validate enforce option
  if (
    plugin.enforce !== undefined &&
    plugin.enforce !== "pre" &&
    plugin.enforce !== "post"
  ) {
    throw new Error(
      `Vbundle plugin '${plugin.name}' has invalid 'enforce' value: ${plugin.enforce}`,
    );
  }

  plugins.push(plugin);
  installedPluginNames.add(plugin.name);

  return {
    name: plugin.name,
    extensions: plugin.extensions ?? [],
    hasResolve: typeof plugin.resolveId === "function",
    hasLoad: typeof plugin.load === "function",
    hasTransform: typeof plugin.transform === "function",
    hasBuildStart: typeof plugin.buildStart === "function",
    hasBuildEnd: typeof plugin.buildEnd === "function",
    hasRenderChunk: typeof plugin.renderChunk === "function",
    hasGenerateBundle: typeof plugin.generateBundle === "function",
    enforce: plugin.enforce,
  };
}

/**
 * Call buildStart hooks on all plugins.
 * @returns {Promise<void>}
 */
export async function buildStart() {
  for (const plugin of sortedPlugins) {
    if (typeof plugin.buildStart !== "function") {
      continue;
    }

    try {
      await plugin.buildStart();
    } catch (err) {
      throw new Error(`Plugin '${plugin.name}' buildStart hook failed`, {
        cause: err,
      });
    }
  }
}

/**
 * Call buildEnd hooks on all plugins.
 * @returns {Promise<void>}
 */
export async function buildEnd() {
  for (const plugin of sortedPlugins) {
    if (typeof plugin.buildEnd !== "function") {
      continue;
    }

    try {
      await plugin.buildEnd();
    } catch (err) {
      throw new Error(`Plugin '${plugin.name}' buildEnd hook failed`, {
        cause: err,
      });
    }
  }
}

/**
 * Call resolveId hooks on all plugins.
 * Returns the first non-null result.
 *
 * @param {string} source - The import specifier to resolve
 * @param {string | null} importer - The module that is importing
 * @param {ResolveOptions} options - Resolution options
 * @returns {Promise<ResolveResult | null>}
 */
export async function resolveId(source, importer, options) {
  for (const plugin of sortedPlugins) {
    if (typeof plugin.resolveId !== "function") {
      continue;
    }

    try {
      let result = plugin.resolveId(source, importer, options);

      // Await if promise
      if (result && typeof result === "object" && "then" in result) {
        result = await result;
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
 * @returns {Promise<LoadResult | null>}
 */
export async function load(id) {
  for (const plugin of sortedPlugins) {
    if (typeof plugin.load !== "function") {
      continue;
    }

    try {
      let result = plugin.load(id);

      // Await if promise
      if (result && typeof result === "object" && "then" in result) {
        result = await result;
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
 * @returns {Promise<TransformResult | null>}
 */
export async function transform(id, code) {
  let currentCode = code;
  let combinedMap = null;
  let hasTransformed = false;

  for (const plugin of sortedPlugins) {
    if (typeof plugin.transform !== "function") {
      continue;
    }

    try {
      let result = plugin.transform(currentCode, id);

      // Await if promise
      if (result && typeof result === "object" && "then" in result) {
        result = await result;
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
        { cause: err },
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
 * Call renderChunk hooks on all plugins.
 * Each plugin can transform the chunk code, and transformations are chained.
 *
 * @param {string} code - The chunk code
 * @param {ChunkInfo} chunk - Information about the chunk
 * @returns {Promise<RenderChunkResult | null>}
 */
export async function renderChunk(code, chunk) {
  let currentCode = code;
  let combinedMap = null;
  let hasTransformed = false;

  for (const plugin of sortedPlugins) {
    if (typeof plugin.renderChunk !== "function") {
      continue;
    }

    try {
      let result = plugin.renderChunk(currentCode, chunk);

      // Await if promise
      if (result && typeof result === "object" && "then" in result) {
        result = await result;
      }

      if (result != null) {
        hasTransformed = true;

        // Normalize result
        if (typeof result === "string") {
          currentCode = result;
        } else {
          currentCode = result.code;
          if (result.map) {
            combinedMap = result.map;
          }
        }
      }
    } catch (err) {
      throw new Error(
        `Plugin '${plugin.name}' renderChunk hook failed for '${chunk.fileName}'`,
        { cause: err },
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
 * Call generateBundle hooks on all plugins.
 *
 * @param {Record<string, ChunkInfo>} bundle - The bundle being generated
 * @returns {Promise<void>}
 */
export async function generateBundle(bundle) {
  for (const plugin of sortedPlugins) {
    if (typeof plugin.generateBundle !== "function") {
      continue;
    }

    try {
      await plugin.generateBundle(bundle);
    } catch (err) {
      throw new Error(`Plugin '${plugin.name}' generateBundle hook failed`, {
        cause: err,
      });
    }
  }
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
internals.buildStart = buildStart;
internals.buildEnd = buildEnd;
internals.resolveId = resolveId;
internals.load = load;
internals.transform = transform;
internals.renderChunk = renderChunk;
internals.generateBundle = generateBundle;

// Import built-in plugins
import {
  jsonPlugin,
  definePlugin,
  aliasPlugin,
  virtualPlugin,
  importGlobPlugin,
  builtinPlugins,
} from "ext:cli/41_vbundle_plugins.js";

// Also expose on Deno namespace for plugin authors
if (!Deno.vbundle) {
  Deno.vbundle = {};
}

Deno.vbundle.emitFile = emitFile;
Deno.vbundle.PluginContext = PluginContext;

// Built-in plugin factories
Deno.vbundle.plugins = {
  json: jsonPlugin,
  define: definePlugin,
  alias: aliasPlugin,
  virtual: virtualPlugin,
  importGlob: importGlobPlugin,
};

// Also export for internal use
export { builtinPlugins };
