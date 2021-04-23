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
   * @property {"esm"=} bundle
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
   * @property {"esm"=} bundle
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

  window.__bootstrap.compilerApi = {
    emit,
  };
})(this);
