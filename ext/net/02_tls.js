// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
const { internalRidSymbol } = core;
import {
  op_net_accept_tls,
  op_net_connect_tls,
  op_net_listen_tls,
  op_tls_cert_resolver_create,
  op_tls_cert_resolver_poll,
  op_tls_cert_resolver_resolve,
  op_tls_cert_resolver_resolve_error,
  op_tls_handshake,
  op_tls_key_null,
  op_tls_key_static,
  op_tls_start,
} from "ext:core/ops";
const {
  ObjectDefineProperty,
  TypeError,
  SymbolFor,
} = primordials;

import { Conn, Listener, validatePort } from "ext:deno_net/01_net.js";

class TlsConn extends Conn {
  #rid = 0;

  constructor(rid, remoteAddr, localAddr) {
    super(rid, remoteAddr, localAddr);
    ObjectDefineProperty(this, internalRidSymbol, {
      __proto__: null,
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
  }

  handshake() {
    return op_tls_handshake(this.#rid);
  }
}

async function connectTls({
  port,
  hostname = "127.0.0.1",
  transport = "tcp",
  caCerts = [],
  alpnProtocols = undefined,
  keyFormat = undefined,
  cert = undefined,
  key = undefined,
}) {
  if (transport !== "tcp") {
    throw new TypeError(`Unsupported transport: '${transport}'`);
  }

  const keyPair = loadTlsKeyPair("Deno.connectTls", {
    keyFormat,
    cert,
    key,
  });
  // TODO(mmastrac): We only expose this feature via symbol for now. This should actually be a feature
  // in Deno.connectTls, however.
  const serverName = arguments[0][serverNameSymbol] ?? null;
  const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_connect_tls(
    { hostname, port },
    { caCerts, alpnProtocols, serverName },
    keyPair,
  );
  localAddr.transport = "tcp";
  remoteAddr.transport = "tcp";
  return new TlsConn(rid, remoteAddr, localAddr);
}

class TlsListener extends Listener {
  #rid = 0;

  constructor(rid, addr) {
    super(rid, addr);
    ObjectDefineProperty(this, internalRidSymbol, {
      __proto__: null,
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
  }

  async accept() {
    const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_accept_tls(
      this.#rid,
    );
    localAddr.transport = "tcp";
    remoteAddr.transport = "tcp";
    return new TlsConn(rid, remoteAddr, localAddr);
  }
}

/**
 * Returns true if this object has the shape of one of the certified key material
 * interfaces.
 */
function hasTlsKeyPairOptions(options) {
  // TODO(mmastrac): remove this temporary symbol when the API lands
  if (options[resolverSymbol] !== undefined) {
    return true;
  }
  return (options.cert !== undefined || options.key !== undefined);
}

/**
 * Loads a TLS keypair from one of the various options. If no key material is provided,
 * returns a special Null keypair.
 */
function loadTlsKeyPair(api, {
  keyFormat,
  cert,
  key,
}) {
  // TODO(mmastrac): remove this temporary symbol when the API lands
  if (arguments[1][resolverSymbol] !== undefined) {
    return createTlsKeyResolver(arguments[1][resolverSymbol]);
  }

  // Check for "pem" format
  if (keyFormat !== undefined && keyFormat !== "pem") {
    throw new TypeError(
      `If "keyFormat" is specified, it must be "pem": received "${keyFormat}"`,
    );
  }

  if (cert !== undefined && key === undefined) {
    throw new TypeError(
      `If \`cert\` is specified, \`key\` must be specified as well for \`${api}\``,
    );
  }
  if (cert === undefined && key !== undefined) {
    throw new TypeError(
      `If \`key\` is specified, \`cert\` must be specified as well for \`${api}\``,
    );
  }

  if (cert !== undefined) {
    return op_tls_key_static(cert, key);
  } else {
    return op_tls_key_null();
  }
}

function listenTls({
  port,
  hostname = "0.0.0.0",
  transport = "tcp",
  alpnProtocols = undefined,
  reusePort = false,
}) {
  if (transport !== "tcp") {
    throw new TypeError(`Unsupported transport: '${transport}'`);
  }
  port = validatePort(port);

  if (!hasTlsKeyPairOptions(arguments[0])) {
    throw new TypeError(
      "A key and certificate are required for `Deno.listenTls`",
    );
  }
  const keyPair = loadTlsKeyPair("Deno.listenTls", arguments[0]);
  const { 0: rid, 1: localAddr } = op_net_listen_tls(
    { hostname, port },
    { alpnProtocols, reusePort },
    keyPair,
  );
  return new TlsListener(rid, localAddr);
}

// deno-lint-ignore require-await
async function startTls(
  conn,
  {
    hostname = "127.0.0.1",
    caCerts = [],
    alpnProtocols = undefined,
  } = { __proto__: null },
) {
  const { 0: rid, 1: localAddr, 2: remoteAddr } = op_tls_start({
    rid: conn[internalRidSymbol],
    hostname,
    caCerts,
    alpnProtocols,
  });
  return new TlsConn(rid, remoteAddr, localAddr);
}

const resolverSymbol = SymbolFor("unstableSniResolver");
const serverNameSymbol = SymbolFor("unstableServerName");

function createTlsKeyResolver(callback) {
  const { 0: resolver, 1: lookup } = op_tls_cert_resolver_create();
  (async () => {
    while (true) {
      const sni = await op_tls_cert_resolver_poll(lookup);
      if (typeof sni !== "string") {
        break;
      }
      try {
        const key = await callback(sni);
        if (!hasTlsKeyPairOptions(key)) {
          op_tls_cert_resolver_resolve_error(lookup, sni, "Invalid key");
        } else {
          const resolved = loadTlsKeyPair("Deno.listenTls", key);
          op_tls_cert_resolver_resolve(lookup, sni, resolved);
        }
      } catch (e) {
        op_tls_cert_resolver_resolve_error(lookup, sni, e.message);
      }
    }
  })();
  return resolver;
}

internals.resolverSymbol = resolverSymbol;
internals.serverNameSymbol = serverNameSymbol;
internals.createTlsKeyResolver = createTlsKeyResolver;

export {
  connectTls,
  hasTlsKeyPairOptions,
  listenTls,
  loadTlsKeyPair,
  startTls,
  TlsConn,
  TlsListener,
};
