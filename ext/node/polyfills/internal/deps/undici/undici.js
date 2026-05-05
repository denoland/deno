// Copyright 2018-2026 the Deno authors. MIT license.
(function () {
const { core, internals, primordials } = globalThis.__bootstrap;
const { isArrayBufferView } = core.loadExtScript(
  "ext:deno_node/internal/util/types.ts",
);
const { TextDecoder } = core.loadExtScript(
  "ext:deno_web/08_text_encoding.js",
);

const {
  ArrayIsArray,
  ArrayPrototypeMap,
  SymbolFor,
  TypeError,
  Uint8Array,
} = primordials;
const { isAnyArrayBuffer } = core;

const kDispatcherOptions = SymbolFor(
  "Deno.internal.node.undici.dispatcherOptions",
);
const kGlobalDispatcher = SymbolFor(
  "Deno.internal.node.undici.globalDispatcher",
);

function normalizeCaCerts(ca) {
  const certs = ArrayIsArray(ca) ? ca : [ca];
  return ArrayPrototypeMap(certs, (cert) => {
    if (typeof cert === "string") {
      return cert;
    }
    if (isArrayBufferView(cert)) {
      return new TextDecoder().decode(cert);
    }
    if (isAnyArrayBuffer(cert)) {
      return new TextDecoder().decode(new Uint8Array(cert));
    }
    throw new TypeError(
      "Dispatcher connect.ca must be a string, Buffer, or ArrayBuffer",
    );
  });
}

class Agent {
  constructor(options = { __proto__: null }) {
    const connect = options.connect ?? { __proto__: null };
    const dispatcherOptions = { __proto__: null };

    if (connect.rejectUnauthorized === false) {
      dispatcherOptions.unsafelyIgnoreCertificateErrors = true;
    }

    if (connect.ca !== undefined) {
      dispatcherOptions.caCerts = normalizeCaCerts(connect.ca);
    }

    this[kDispatcherOptions] = dispatcherOptions;
  }
}

function getGlobalDispatcher() {
  return internals[kGlobalDispatcher];
}

function setGlobalDispatcher(dispatcher) {
  internals[kGlobalDispatcher] = dispatcher;
}

const EnvHttpProxyAgent = Agent;

const _defaultExport = {
  Agent,
  EnvHttpProxyAgent,
  getGlobalDispatcher,
  setGlobalDispatcher,
};

return {
  Agent,
  EnvHttpProxyAgent,
  getGlobalDispatcher,
  kDispatcherOptions,
  kGlobalDispatcher,
  setGlobalDispatcher,
  default: _defaultExport,
};
})();
