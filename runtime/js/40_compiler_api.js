// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check

// This file contains the runtime APIs which will dispatch work to the internal
// compiler within Deno.
"use strict";
((window) => {
  const core = window.Deno.core;
  const util = window.__bootstrap.util;

  /**
   * @typedef {object} ImportMap
   * @property {Record<string, string>} imports
   * @property {Record<string, Record<string, string>>=} scopes
   */

  /**
   * @typedef {object} OpEmitRequest
   * @property {"module" | "classic"=} bundle
   * @property {boolean=} check
   * @property {Record<string, any>=} compilerOptions
   * @property {ImportMap=} importMap
   * @property {string=} importMapPath
   * @property {string} rootSpecifier
   * @property {Record<string, string>=} sources
   */

  /**
   * @typedef OpEmitResponse
   * @property {any[]} diagnostics
   * @property {Record<string, string>} files
   * @property {string[]=} ignoredOptions
   * @property {Array<[string, number]>} stats
   */

  /**
   * @param {OpEmitRequest} request
   * @returns {Promise<OpEmitResponse>}
   */
  function opEmit(request) {
    return core.opAsync("op_emit", request);
  }

  /**
   * @typedef {object} OpInfoImportMap
   * @property {Record<string, string>} imports
   * @property {Record<string, Record<string, string>>=} scopes
   */

  /**
   * @typedef {object} OpInfoRequestOptions
   * @property {boolean=} checksums
   * @property {(string | OpInfoImportMap)=} importMap
   * @property {boolean=} paths
   */

  /**
   * @typedef {object} OpInfoRequest
   * @property {string} specifier
   * @property {OpInfoRequestOptions=} options
   */

  /**
   * @typedef {object} OpInfoResponseDependency
   * @property {string} specifier
   * @property {boolean} isDynamic
   * @property {string=} code
   * @property {string=} type
   */

  /**
   * @typedef {object} OpInfoResponseModule
   * @property {string} specifier
   * @property {Array<OpInfoResponseDependency>=} dependencies
   * @property {number=} size
   * @property {mediaType=} string
   * @property {string=} local
   * @property {string=} checksum
   * @property {string=} emit
   * @property {string=} map
   * @property {string=} error
   */

  /**
   * @typedef {object} OpInfoResponse
   * @property {string} root
   * @property {OpInfoResponseModule[]} modules
   * @property {number} size
   */

  /**
   * @param {OpInfoRequest} request
   * @returns {Promise<OpInfoResponse>}
   */
  function opInfo(request) {
    return core.opAsync("op_info", request);
  }

  /**
   * @param {string} specifier
   * @returns {string}
   */
  function checkRelative(specifier) {
    return specifier.match(/^([\.\/\\]|https?:\/{2}|file:\/{2})/)
      ? specifier
      : `./${specifier}`;
  }

  /**
   * @typedef {object} EmitOptions
   * @property {"module" | "classic"=} bundle
   * @property {boolean=} check
   * @property {Record<string, any>=} compilerOptions
   * @property {ImportMap=} importMap
   * @property {string=} importMapPath
   * @property {Record<string, string>=} sources
   */

  /**
   * @param {string | URL} rootSpecifier
   * @param {EmitOptions=} options
   * @returns {Promise<OpEmitResponse>}
   */
  function emit(rootSpecifier, options = {}) {
    util.log(`Deno.emit`, { rootSpecifier });
    if (!rootSpecifier) {
      return Promise.reject(
        new TypeError("A root specifier must be supplied."),
      );
    }
    if (!(typeof rootSpecifier === "string")) {
      rootSpecifier = rootSpecifier.toString();
    }
    if (!options.sources) {
      rootSpecifier = checkRelative(rootSpecifier);
    }
    return opEmit({ rootSpecifier, ...options });
  }

  /**
   * @param {string | URL} specifier
   * @param {object=} options
   * @returns {Promise<OpInfoResponse>}
   */
  function info(specifier, options) {
    util.log(`Deno.info`, { specifier });
    if (!specifier) {
      return Promise.reject(
        new TypeError("A root specifier must be supplied."),
      );
    }
    if (typeof specifier !== "string") {
      specifier = String(specifier);
    }
    if (
      options && options.importMap && typeof options.importMap !== "string" &&
      !("imports" in options.importMap)
    ) {
      options.importMap = String(options.importMap);
    }
    return opInfo({ specifier, options });
  }

  // These correspond to the cli::media_type::MediaType display trait
  var ModuleGraphMediaType;
  (function (ModuleGraphMediaType) {
    ModuleGraphMediaType["JavaScript"] = "JavaScript";
    ModuleGraphMediaType["TypeScript"] = "TypeScript";
    ModuleGraphMediaType["JSX"] = "JSX";
    ModuleGraphMediaType["TSX"] = "TSX";
    ModuleGraphMediaType["Dts"] = "Dts";
    ModuleGraphMediaType["Json"] = "Json";
    ModuleGraphMediaType["Wasm"] = "Wasm";
    ModuleGraphMediaType["TsBuildInfo"] = "TsBuildInfo";
    ModuleGraphMediaType["SourceMap"] = "SourceMap";
    ModuleGraphMediaType["Unknown"] = "Unknown";
  })(ModuleGraphMediaType || (ModuleGraphMediaType = {}));

  window.__bootstrap.compilerApi = {
    emit,
    info,
    ModuleGraphMediaType,
  };
})(this);
