// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const m = Deno[Deno.internal].require.Module;
export const _cache = m._cache;
export const _extensions = m._extensions;
export const _findPath = m._findPath;
export const _initPaths = m._initPaths;
export const _load = m._load;
export const _nodeModulePaths = m._nodeModulePaths;
export const _pathCache = m._pathCache;
export const _preloadModules = m._preloadModules;
export const _resolveFilename = m._resolveFilename;
export const _resolveLookupPaths = m._resolveLookupPaths;
export const builtinModules = m.builtinModules;
export const createRequire = m.createRequire;
export const globalPaths = m.globalPaths;
export const Module = m.Module;
export const wrap = m.wrap;
export default m;
