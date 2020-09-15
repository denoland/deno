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
    const { diagnostics, emitMap } = await opCompile(payload);
    util.assert(emitMap);

    const processedEmitMap = {};
    for (const [key, emittedSource] of Object.entries(emitMap)) {
      processedEmitMap[key] = emittedSource.contents;
    }

    return {
      diagnostics: diagnostics.length === 0 ? undefined : diagnostics,
      emitMap: processedEmitMap,
    };
  }

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
    const { diagnostics, output } = await opCompile(payload);
    util.assert(output);
    return {
      diagnostics: diagnostics.length === 0 ? undefined : diagnostics,
      emitMap: processedEmitMap,
    };
  }

  window.__bootstrap.compilerApi = {
    bundle,
    compile,
    transpileOnly,
  };
})(this);
