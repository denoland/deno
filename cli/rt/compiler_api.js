// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/compiler/api.ts",
  ["$deno$/util.ts", "$deno$/ops/runtime_compiler.ts"],
  function (exports_26, context_26) {
    "use strict";
    let util, runtimeCompilerOps;
    const __moduleName = context_26 && context_26.id;
    function checkRelative(specifier) {
      return specifier.match(/^([\.\/\\]|https?:\/{2}|file:\/{2})/)
        ? specifier
        : `./${specifier}`;
    }
    async function transpileOnly(sources, options = {}) {
      util.log("Deno.transpileOnly", {
        sources: Object.keys(sources),
        options,
      });
      const payload = {
        sources,
        options: JSON.stringify(options),
      };
      const result = await runtimeCompilerOps.transpile(payload);
      return JSON.parse(result);
    }
    exports_26("transpileOnly", transpileOnly);
    async function compile(rootName, sources, options = {}) {
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
      const result = await runtimeCompilerOps.compile(payload);
      return JSON.parse(result);
    }
    exports_26("compile", compile);
    async function bundle(rootName, sources, options = {}) {
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
      const result = await runtimeCompilerOps.compile(payload);
      return JSON.parse(result);
    }
    exports_26("bundle", bundle);
    return {
      setters: [
        function (util_4) {
          util = util_4;
        },
        function (runtimeCompilerOps_1) {
          runtimeCompilerOps = runtimeCompilerOps_1;
        },
      ],
      execute: function () {},
    };
  }
);
