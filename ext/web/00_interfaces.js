// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

"use strict";

((window) => {
  const core = window.Deno.core;
  const { ArrayPrototypePush } = window.__bootstrap.primordials;

  let registeredInterfaces = [];

  function registerInterface(interfaceDict) {
    if (registeredInterfaces === null) {
      core.registerInterface(interfaceDict);
    } else {
      ArrayPrototypePush(registeredInterfaces, interfaceDict);
    }
  }

  function initInterfaces() {
    if (registeredInterfaces !== null) {
      for (const interfaceDict of registeredInterfaces) {
        core.registerInterface(interfaceDict);
      }
      registeredInterfaces = null;
    }
  }

  window.__bootstrap ??= {};
  window.__bootstrap.interfaces = {
    registerInterface,
    initInterfaces,
  };
})(globalThis);
