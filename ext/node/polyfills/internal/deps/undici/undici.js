// Copyright 2018-2026 the Deno authors. MIT license.

import { createHttpClient } from "ext:deno_fetch/22_http_client.js";

let globalDispatcher;

class EnvHttpProxyAgent {
  #client;

  constructor(options = { __proto__: null }) {
    const connect = options.connect ?? { __proto__: null };
    const clientOptions = { __proto__: null };

    if (connect.rejectUnauthorized === false) {
      clientOptions.unsafelyIgnoreCertificateErrors = [];
    }

    if (connect.ca !== undefined) {
      clientOptions.caCerts = Array.isArray(connect.ca)
        ? connect.ca
        : [connect.ca];
    }

    this.#client = createHttpClient(clientOptions);
  }

  get client() {
    return this.#client;
  }

  close() {
    this.#client.close();
  }
}

function getGlobalDispatcher() {
  return globalDispatcher;
}

function setGlobalDispatcher(dispatcher) {
  globalDispatcher = dispatcher;
}

export { EnvHttpProxyAgent, getGlobalDispatcher, setGlobalDispatcher };

export default {
  EnvHttpProxyAgent,
  getGlobalDispatcher,
  setGlobalDispatcher,
};
