// Copyright 2018-2026 the Deno authors. MIT license.

const kDispatcherOptions = Symbol.for(
  "Deno.internal.node.undici.dispatcherOptions",
);
const kGlobalDispatcher = Symbol.for(
  "Deno.internal.node.undici.globalDispatcher",
);

function normalizeCaCerts(ca) {
  const certs = Array.isArray(ca) ? ca : [ca];
  return certs.map((cert) => {
    if (typeof cert === "string") {
      return cert;
    }
    if (ArrayBuffer.isView(cert)) {
      return new TextDecoder().decode(cert);
    }
    if (cert instanceof ArrayBuffer) {
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
  return globalThis[kGlobalDispatcher];
}

function setGlobalDispatcher(dispatcher) {
  globalThis[kGlobalDispatcher] = dispatcher;
}

const EnvHttpProxyAgent = Agent;

export {
  Agent,
  EnvHttpProxyAgent,
  getGlobalDispatcher,
  kDispatcherOptions,
  kGlobalDispatcher,
  setGlobalDispatcher,
};

export default {
  Agent,
  EnvHttpProxyAgent,
  getGlobalDispatcher,
  setGlobalDispatcher,
};
