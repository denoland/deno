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
  op_tls_key_static_from_file,
  op_tls_start,
} from "ext:core/ops";
const {
  Number,
  ObjectDefineProperty,
  TypeError,
  SymbolFor,
} = primordials;

import { Conn, Listener } from "ext:deno_net/01_net.js";

class TlsConn extends Conn {
  #rid = 0;

  constructor(rid, remoteAddr, localAddr, preventCloseOnEOF) {
    super(rid, remoteAddr, localAddr, preventCloseOnEOF);
    ObjectDefineProperty(this, internalRidSymbol, {
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.TlsConn.rid",
      new Error().stack,
      "Use `Deno.TlsConn` instance methods instead.",
    );
    return this.#rid;
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
  certFile = undefined,
  certChain = undefined,
  key = undefined,
  keyFile = undefined,
  privateKey = undefined,
  preventCloseOnEOF = false,
}) {
  if (transport !== "tcp") {
    throw new TypeError(`Unsupported transport: '${transport}'`);
  }
  let deprecatedCertFile = undefined;

  // Deno.connectTls has an irregular option where you can just pass `certFile` and
  // not `keyFile`. In this case it's used for `caCerts` rather than the client key.
  if (certFile !== undefined && keyFile === undefined) {
    internals.warnOnDeprecatedApi(
      "Deno.ConnectTlsOptions.certFile",
      new Error().stack,
      "Pass the cert file's contents to the `Deno.ConnectTlsOptions.caCerts` option instead.",
    );

    deprecatedCertFile = certFile;
    certFile = undefined;
  }

  const keyPair = loadTlsKeyPair("Deno.connectTls", {
    keyFormat,
    cert,
    certFile,
    certChain,
    key,
    keyFile,
    privateKey,
  });
  // TODO(mmastrac): We only expose this feature via symbol for now. This should actually be a feature
  // in Deno.connectTls, however.
  const serverName = arguments[0][serverNameSymbol] ?? null;
  const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_connect_tls(
    { hostname, port },
    { certFile: deprecatedCertFile, caCerts, alpnProtocols, serverName },
    keyPair,
  );
  localAddr.transport = "tcp";
  remoteAddr.transport = "tcp";
  return new TlsConn(rid, remoteAddr, localAddr, preventCloseOnEOF);
}

class TlsListener extends Listener {
  #rid = 0;

  constructor(rid, addr) {
    super(rid, addr);
    ObjectDefineProperty(this, internalRidSymbol, {
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.TlsListener.rid",
      new Error().stack,
      "Use `Deno.TlsListener` instance methods instead.",
    );
    return this.#rid;
  }

  async accept({ preventCloseOnEOF = false } = { __proto__: null }) {
    const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_accept_tls(
      this.#rid,
    );
    localAddr.transport = "tcp";
    remoteAddr.transport = "tcp";
    return new TlsConn(rid, remoteAddr, localAddr, preventCloseOnEOF);
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
  return (options.cert !== undefined || options.key !== undefined ||
    options.certFile !== undefined ||
    options.keyFile !== undefined || options.privateKey !== undefined ||
    options.certChain !== undefined);
}

/**
 * Loads a TLS keypair from one of the various options. If no key material is provided,
 * returns a special Null keypair.
 */
function loadTlsKeyPair(api, {
  keyFormat,
  cert,
  certFile,
  certChain,
  key,
  keyFile,
  privateKey,
}) {
  if (internals.future) {
    certFile = undefined;
    certChain = undefined;
    keyFile = undefined;
    privateKey = undefined;
  }

  // TODO(mmastrac): remove this temporary symbol when the API lands
  if (arguments[1][resolverSymbol] !== undefined) {
    return createTlsKeyResolver(arguments[1][resolverSymbol]);
  }

  // Check for "pem" format
  if (keyFormat !== undefined && keyFormat !== "pem") {
    throw new TypeError('If `keyFormat` is specified, it must be "pem"');
  }

  function exclusive(a1, a1v, a2, a2v) {
    if (a1v !== undefined && a2v !== undefined) {
      throw new TypeError(
        `Cannot specify both \`${a1}\` and \`${a2}\` for \`${api}\`.`,
      );
    }
  }

  // Ensure that only one pair is valid
  exclusive("certChain", certChain, "cert", cert);
  exclusive("certChain", certChain, "certFile", certFile);
  exclusive("key", key, "keyFile", keyFile);
  exclusive("key", key, "privateKey", privateKey);

  function both(a1, a1v, a2, a2v) {
    if (a1v !== undefined && a2v === undefined) {
      throw new TypeError(
        `If \`${a1}\` is specified, \`${a2}\` must be specified as well for \`${api}\`.`,
      );
    }
    if (a1v === undefined && a2v !== undefined) {
      throw new TypeError(
        `If \`${a2}\` is specified, \`${a1}\` must be specified as well for \`${api}\`.`,
      );
    }
  }

  // Pick one pair of cert/key, certFile/keyFile or certChain/privateKey
  both("cert", cert, "key", key);
  both("certFile", certFile, "keyFile", keyFile);
  both("certChain", certChain, "privateKey", privateKey);

  if (certFile !== undefined) {
    internals.warnOnDeprecatedApi(
      "Deno.TlsCertifiedKeyOptions.keyFile",
      new Error().stack,
      "Pass the key file's contents to the `Deno.TlsCertifiedKeyPem.key` option instead.",
    );
    internals.warnOnDeprecatedApi(
      "Deno.TlsCertifiedKeyOptions.certFile",
      new Error().stack,
      "Pass the cert file's contents to the `Deno.TlsCertifiedKeyPem.cert` option instead.",
    );
    return op_tls_key_static_from_file(api, certFile, keyFile);
  } else if (certChain !== undefined) {
    if (api !== "Deno.connectTls") {
      throw new TypeError(
        `Invalid options 'certChain' and 'privateKey' for ${api}`,
      );
    }
    internals.warnOnDeprecatedApi(
      "Deno.TlsCertifiedKeyOptions.privateKey",
      new Error().stack,
      "Use the `Deno.TlsCertifiedKeyPem.key` option instead.",
    );
    internals.warnOnDeprecatedApi(
      "Deno.TlsCertifiedKeyOptions.certChain",
      new Error().stack,
      "Use the `Deno.TlsCertifiedKeyPem.cert` option instead.",
    );
    return op_tls_key_static(certChain, privateKey);
  } else if (cert !== undefined) {
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

  if (!hasTlsKeyPairOptions(arguments[0])) {
    throw new TypeError(
      "A key and certificate are required for `Deno.listenTls`",
    );
  }
  const keyPair = loadTlsKeyPair("Deno.listenTls", arguments[0]);
  const { 0: rid, 1: localAddr } = op_net_listen_tls(
    { hostname, port: Number(port) },
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
    preventCloseOnEOF = false,
  } = { __proto__: null },
) {
  const { 0: rid, 1: localAddr, 2: remoteAddr } = op_tls_start({
    rid: conn[internalRidSymbol],
    hostname,
    caCerts,
    alpnProtocols,
  });
  return new TlsConn(rid, remoteAddr, localAddr, preventCloseOnEOF);
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
