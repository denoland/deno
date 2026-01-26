// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check

/**
 * Built-in plugins for the vbundle system.
 *
 * These plugins provide common functionality that most bundler setups need:
 * - JSON: Import JSON files as ES modules
 * - Define: Replace global constants at build time
 * - Alias: Path aliasing for cleaner imports
 */

/**
 * JSON plugin - transforms JSON files into ES modules.
 *
 * @example
 * ```ts
 * import data from './config.json';
 * console.log(data.version);
 * ```
 *
 * @returns {import('./40_vbundle.js').Plugin}
 */
export function jsonPlugin() {
  return {
    name: "vbundle:json",
    extensions: [".json"],

    transform(code, id) {
      if (!id.endsWith(".json")) {
        return null;
      }

      try {
        // Parse to validate JSON and get the value
        const parsed = JSON.parse(code);

        // Convert to ES module export
        const output = `export default ${JSON.stringify(parsed, null, 2)};`;

        return {
          code: output,
          map: undefined,
        };
      } catch (err) {
        throw new Error(`Failed to parse JSON file '${id}': ${err.message}`);
      }
    },
  };
}

/**
 * Define plugin - replaces global identifiers with constant values.
 *
 * @example
 * ```ts
 * // Plugin config
 * definePlugin({
 *   'process.env.NODE_ENV': '"production"',
 *   '__DEV__': 'false',
 *   'import.meta.env.MODE': '"production"',
 * })
 *
 * // In your code
 * if (process.env.NODE_ENV === 'production') { ... }
 * // Becomes
 * if ("production" === 'production') { ... }
 * ```
 *
 * @param {Record<string, string>} definitions - Map of identifiers to replacement values
 * @returns {import('./40_vbundle.js').Plugin}
 */
export function definePlugin(definitions) {
  // Sort definitions by length (longest first) to avoid partial replacements
  const sortedKeys = Object.keys(definitions).sort(
    (a, b) => b.length - a.length,
  );

  // Build a regex that matches any of the definitions
  // Escape special regex characters in keys
  const escapedKeys = sortedKeys.map((key) =>
    key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
  );

  if (escapedKeys.length === 0) {
    // No definitions, return a no-op plugin
    return {
      name: "vbundle:define",
      transform() {
        return null;
      },
    };
  }

  // Match whole words/expressions only
  const pattern = new RegExp(
    `\\b(${escapedKeys.join("|")})\\b`,
    "g",
  );

  return {
    name: "vbundle:define",

    transform(code, id) {
      // Skip non-JS/TS files
      if (
        !id.endsWith(".js") &&
        !id.endsWith(".ts") &&
        !id.endsWith(".jsx") &&
        !id.endsWith(".tsx") &&
        !id.endsWith(".mjs") &&
        !id.endsWith(".mts")
      ) {
        return null;
      }

      // Check if any definitions exist in the code
      if (!sortedKeys.some((key) => code.includes(key))) {
        return null;
      }

      // Replace all occurrences
      const transformed = code.replace(pattern, (match) => {
        return definitions[match] ?? match;
      });

      if (transformed === code) {
        return null;
      }

      return {
        code: transformed,
        map: undefined, // TODO: Generate source map for replacements
      };
    },
  };
}

/**
 * Alias plugin - resolves import path aliases.
 *
 * @example
 * ```ts
 * // Plugin config
 * aliasPlugin({
 *   '@/': './src/',
 *   '~': './src',
 *   'lodash': 'lodash-es',
 * })
 *
 * // In your code
 * import { foo } from '@/utils/foo';
 * // Resolves to './src/utils/foo'
 * ```
 *
 * @param {Record<string, string>} aliases - Map of alias prefixes to replacement paths
 * @returns {import('./40_vbundle.js').Plugin}
 */
export function aliasPlugin(aliases) {
  // Sort aliases by length (longest first) to match most specific first
  const sortedAliases = Object.entries(aliases).sort(
    ([a], [b]) => b.length - a.length,
  );

  return {
    name: "vbundle:alias",
    enforce: "pre", // Run before other plugins

    resolveId(source, importer, _options) {
      // Try each alias
      for (const [alias, replacement] of sortedAliases) {
        if (source === alias) {
          // Exact match
          return { id: replacement };
        }

        if (source.startsWith(alias)) {
          // Prefix match (e.g., '@/' matches '@/foo')
          const rest = source.slice(alias.length);
          const resolved = replacement + rest;
          return { id: resolved };
        }
      }

      // No alias matched
      return null;
    },
  };
}

/**
 * Virtual module plugin - provides virtual modules that don't exist on disk.
 *
 * @example
 * ```ts
 * // Plugin config
 * virtualPlugin({
 *   'virtual:config': 'export const apiUrl = "https://api.example.com";',
 *   '\0virtual:env': `export const MODE = "${Deno.env.get('MODE')}";`,
 * })
 *
 * // In your code
 * import { apiUrl } from 'virtual:config';
 * ```
 *
 * @param {Record<string, string>} modules - Map of virtual module ids to their source code
 * @returns {import('./40_vbundle.js').Plugin}
 */
export function virtualPlugin(modules) {
  const virtualIds = new Set(Object.keys(modules));

  // Normalize virtual ids - add \0 prefix if not present
  const normalizedModules = {};
  for (const [id, code] of Object.entries(modules)) {
    const normalizedId = id.startsWith("\0") ? id : `\0${id}`;
    normalizedModules[normalizedId] = code;
    normalizedModules[id] = code; // Keep original too for resolveId
  }

  return {
    name: "vbundle:virtual",
    enforce: "pre",

    resolveId(source, _importer, _options) {
      if (virtualIds.has(source)) {
        // Return with \0 prefix to mark as virtual
        return { id: source.startsWith("\0") ? source : `\0${source}` };
      }
      return null;
    },

    load(id) {
      const code = normalizedModules[id];
      if (code !== undefined) {
        return { code };
      }
      return null;
    },
  };
}

/**
 * Import glob plugin - enables glob pattern imports.
 *
 * @example
 * ```ts
 * // In your code
 * const modules = import.meta.glob('./modules/*.ts');
 * // Becomes an object mapping paths to dynamic imports
 * ```
 *
 * Note: This is a simplified implementation. Full implementation would
 * need to integrate with the file system and bundler graph.
 *
 * @returns {import('./40_vbundle.js').Plugin}
 */
export function importGlobPlugin() {
  return {
    name: "vbundle:import-glob",

    transform(code, id) {
      // Check if code contains import.meta.glob
      if (!code.includes("import.meta.glob")) {
        return null;
      }

      // This is a placeholder - full implementation would:
      // 1. Parse the AST to find import.meta.glob calls
      // 2. Resolve the glob patterns against the file system
      // 3. Generate the appropriate import statements

      // For now, we'll leave this as a no-op and implement fully later
      return null;
    },
  };
}

// Export all built-in plugins
export const builtinPlugins = {
  json: jsonPlugin,
  define: definePlugin,
  alias: aliasPlugin,
  virtual: virtualPlugin,
  importGlob: importGlobPlugin,
};
