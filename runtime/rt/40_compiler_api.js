// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This file contains the runtime APIs which will dispatch work to the internal
// compiler within Deno.
((window) => {
  const core = window.Deno.core;
  const util = window.__bootstrap.util;

  function opCompile(request) {
    return core.jsonOpAsync("op_compile", request);
  }

  function opTranspile(
    request,
  ) {
    return core.jsonOpAsync("op_transpile", request);
  }

  function checkRelative(specifier) {
    return specifier.match(/^([\.\/\\]|https?:\/{2}|file:\/{2})/)
      ? specifier
      : `./${specifier}`;
  }

  // TODO(bartlomieju): change return type to interface?
  function transpileOnly(
    sources,
    options = {},
  ) {
    util.log("Deno.transpileOnly", { sources: Object.keys(sources), options });
    const payload = {
      sources,
      options: JSON.stringify(options),
    };
    return opTranspile(payload);
  }

  // TODO(bartlomieju): change return type to interface?
  async function compile(
    rootName,
    sources,
    options = {},
  ) {
    const payload = {
      rootName: sources ? rootName : checkRelative(rootName),
      sources,
      options: JSON.stringify(options),
      bundle: false,
    };
    util.log("Deno.compile", {
      rootName: payload.rootName,
      sources: !!sources,
      options,
    });
    /** @type {{ emittedFiles: Record<string, string>, diagnostics: any[] }} */
    const result = await opCompile(payload);
    util.assert(result.emittedFiles);
    const maybeDiagnostics = result.diagnostics.length === 0
      ? undefined
      : result.diagnostics;

    return [maybeDiagnostics, result.emittedFiles];
  }

  // TODO(bartlomieju): change return type to interface?
  async function bundle(
    rootName,
    sources,
    options = {},
  ) {
    const payload = {
      rootName: sources ? rootName : checkRelative(rootName),
      sources,
      options: JSON.stringify(options),
      bundle: true,
    };
    util.log("Deno.bundle", {
      rootName: payload.rootName,
      sources: !!sources,
      options,
    });
    /** @type {{ emittedFiles: Record<string, string>, diagnostics: any[] }} */
    const result = await opCompile(payload);
    const output = result.emittedFiles["deno:///bundle.js"];
    util.assert(output);
    const maybeDiagnostics = result.diagnostics.length === 0
      ? undefined
      : result.diagnostics;
    return [maybeDiagnostics, output];
  }

  window.__bootstrap.compilerApi = {
    bundle,
    compile,
    transpileOnly,
  };
})(this);
