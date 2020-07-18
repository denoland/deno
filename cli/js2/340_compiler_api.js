// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This file contains the runtime APIs which will dispatch work to the internal
// compiler within Deno.
((window) => {
  const util = window.__bootstrap.util;
  const { sendAsync } = window.__bootstrap.dispatchJson;

  function opCompile(request) {
    return sendAsync("op_compile", request);
  }

  function opTranspile(
    request,
  ) {
    return sendAsync("op_transpile", request);
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
    const result = await opCompile(payload);
    util.assert(result.emitMap);
    const maybeDiagnostics = result.diagnostics.length === 0
      ? undefined
      : result.diagnostics;

    const emitMap = {};

    for (const [key, emittedSource] of Object.entries(result.emitMap)) {
      emitMap[key] = emittedSource.contents;
    }

    return [maybeDiagnostics, emitMap];
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
    const result = await opCompile(payload);
    util.assert(result.output);
    const maybeDiagnostics = result.diagnostics.length === 0
      ? undefined
      : result.diagnostics;
    return [maybeDiagnostics, result.output];
  }

  window.__bootstrap.compilerApi = {
    bundle,
    compile,
    transpileOnly,
  };
})(this);
