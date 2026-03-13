// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check

import { internals } from "ext:core/mod.js";

/**
 * @typedef {{
 *   filter: string;
 *   namespace?: string;
 *   order?: "pre" | "normal" | "post";
 * }} HookOptions
 *
 * @typedef {{
 *   path: string;
 *   namespace?: string;
 *   external?: boolean;
 * }} ResolveResult
 *
 * @typedef {{
 *   content: string;
 *   loader: string;
 * }} LoadResult
 *
 * @typedef {{
 *   content?: string;
 *   loader?: string;
 *   sourceMap?: string;
 * }} TransformResult
 *
 * @typedef {{
 *   addEntries?: string[];
 *   removeEntries?: string[];
 * }} WatchChangeResult
 */

/** @type {Array<{ filter: RegExp, namespace: string | null, order: string, callback: Function }>} */
const resolveHooks = [];
/** @type {Array<{ filter: RegExp, namespace: string | null, order: string, callback: Function }>} */
const loadHooks = [];
/** @type {Array<{ filter: RegExp, namespace: string | null, order: string, callback: Function }>} */
const transformHooks = [];
/** @type {Array<{ filter: RegExp, namespace: string | null, callback: Function }>} */
const watchChangeHooks = [];

/**
 * @param {Array<{ default: Function }>} pluginModules
 * @returns {{ pluginCount: number, resolveHookCount: number, loadHookCount: number, transformHookCount: number, watchChangeHookCount: number }}
 */
function installPlugins(pluginModules) {
  for (const mod of pluginModules) {
    const pluginFn = mod.default || mod;
    if (typeof pluginFn !== "function") {
      throw new Error(
        "Build plugin must export a default function: (build) => void",
      );
    }

    const build = {
      /**
       * @param {HookOptions} options
       * @param {Function} callback
       */
      onResolve(options, callback) {
        resolveHooks.push({
          filter: new RegExp(options.filter),
          namespace: options.namespace ?? null,
          order: options.order ?? "normal",
          callback,
        });
      },
      /**
       * @param {HookOptions} options
       * @param {Function} callback
       */
      onLoad(options, callback) {
        loadHooks.push({
          filter: new RegExp(options.filter),
          namespace: options.namespace ?? null,
          order: options.order ?? "normal",
          callback,
        });
      },
      /**
       * @param {HookOptions} options
       * @param {Function} callback
       */
      onTransform(options, callback) {
        transformHooks.push({
          filter: new RegExp(options.filter),
          namespace: options.namespace ?? null,
          order: options.order ?? "normal",
          callback,
        });
      },
      /**
       * @param {Function} callback
       */
      onWatchChange(callback) {
        watchChangeHooks.push({
          filter: /.*/,
          namespace: null,
          callback,
        });
      },
    };

    pluginFn(build);
  }

  // Sort hooks by order
  const orderValue = { pre: 0, normal: 1, post: 2 };
  const sortByOrder = (a, b) =>
    (orderValue[a.order] ?? 1) - (orderValue[b.order] ?? 1);
  resolveHooks.sort(sortByOrder);
  loadHooks.sort(sortByOrder);
  transformHooks.sort(sortByOrder);

  return {
    pluginCount: pluginModules.length,
    resolveHookCount: resolveHooks.length,
    loadHookCount: loadHooks.length,
    transformHookCount: transformHooks.length,
    watchChangeHookCount: watchChangeHooks.length,
  };
}

/**
 * @param {string} specifier
 * @param {string} importer
 * @param {string} namespace
 * @param {string} kind
 * @returns {ResolveResult | null}
 */
function runOnResolve(specifier, importer, namespace, kind) {
  for (const hook of resolveHooks) {
    if (hook.namespace !== null && hook.namespace !== namespace) {
      continue;
    }
    if (!hook.filter.test(specifier)) {
      continue;
    }
    const result = hook.callback({
      path: specifier,
      importer,
      namespace,
      kind,
    });
    if (result != null) {
      return {
        path: result.path ?? specifier,
        namespace: result.namespace ?? "file",
        external: result.external ?? false,
      };
    }
  }
  return null;
}

/**
 * @param {string} path
 * @param {string} namespace
 * @returns {LoadResult | null}
 */
function runOnLoad(path, namespace) {
  for (const hook of loadHooks) {
    if (hook.namespace !== null && hook.namespace !== namespace) {
      continue;
    }
    if (!hook.filter.test(path)) {
      continue;
    }
    const result = hook.callback({ path, namespace });
    if (result != null) {
      return {
        content: result.contents ?? result.content ?? "",
        loader: result.loader ?? "js",
      };
    }
  }
  return null;
}

/**
 * @param {string} content
 * @param {string} path
 * @param {string} namespace
 * @param {string} loader
 * @param {string | null} sourceMap
 * @returns {TransformResult | null}
 */
function runOnTransform(content, path, namespace, loader, sourceMap) {
  let currentContent = content;
  let currentLoader = loader;
  let currentSourceMap = sourceMap;
  let anyChanged = false;

  for (const hook of transformHooks) {
    if (hook.namespace !== null && hook.namespace !== namespace) {
      continue;
    }
    if (!hook.filter.test(path)) {
      continue;
    }
    const result = hook.callback({
      content: currentContent,
      path,
      namespace,
      loader: currentLoader,
      sourceMap: currentSourceMap,
    });
    if (result != null) {
      if (result.contents != null || result.content != null) {
        currentContent = result.contents ?? result.content;
        anyChanged = true;
      }
      if (result.loader != null) {
        currentLoader = result.loader;
        anyChanged = true;
      }
      if (result.sourceMap != null) {
        currentSourceMap = result.sourceMap;
        anyChanged = true;
      }
    }
  }

  if (!anyChanged) {
    return null;
  }

  return {
    content: currentContent,
    loader: currentLoader,
    sourceMap: currentSourceMap,
  };
}

/**
 * @param {string} path
 * @returns {WatchChangeResult}
 */
function runOnWatchChange(path) {
  /** @type {string[]} */
  const addEntries = [];
  /** @type {string[]} */
  const removeEntries = [];

  for (const hook of watchChangeHooks) {
    if (!hook.filter.test(path)) {
      continue;
    }
    const result = hook.callback({ path });
    if (result != null) {
      if (result.addEntries) {
        addEntries.push(...result.addEntries);
      }
      if (result.removeEntries) {
        removeEntries.push(...result.removeEntries);
      }
    }
  }

  return { addEntries, removeEntries };
}

// Expose to Rust via Deno internals
internals.installBuildPlugins = installPlugins;
internals.runBuildOnResolve = runOnResolve;
internals.runBuildOnLoad = runOnLoad;
internals.runBuildOnTransform = runOnTransform;
internals.runBuildOnWatchChange = runOnWatchChange;
