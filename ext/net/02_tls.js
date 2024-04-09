// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
const { internalRidSymbol } = core;
import {
  op_net_accept_tls,
  op_net_connect_tls,
  op_net_listen_tls,
  op_tls_handshake,
  op_tls_key_null,
  op_tls_key_static,
  op_tls_key_static_from_file,
  op_tls_start,
} from "ext:core/ops";
const {
  Number,
  ObjectDefineProperty,
  ReflectHas,
  TypeError,
} = primordials;

import { Conn, Listener } from "ext:deno_net/01_net.js";

class TlsConn extends Conn {
  #rid = 0;

  constructor(rid, remoteAddr, localAddr) {
    super(rid, remoteAddr, localAddr);
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
}) {
  if (transport !== "tcp") {
    throw new TypeError(`Unsupported transport: '${transport}'`);
  }
  const keyPair = loadTlsKeyPair("Deno.connectTls", arguments[0]);
  const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_connect_tls(
    { hostname, port },
    { certFile, caCerts, cert, key, alpnProtocols },
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
  return (ReflectHas(options, "cert") || ReflectHas(options, "key") ||
    ReflectHas(options, "certFile") ||
    ReflectHas(options, "keyFile") || ReflectHas(options, "privateKey") ||
    ReflectHas(options, "certChain"));
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
  // Check for "pem" format
  if (keyFormat !== undefined && keyFormat !== "pem") {
    throw new TypeError('If `keyFormat` is specified, it must be "pem"');
  }
  // Pick one pair of cert/key, certFile/keyFile or certChain/privateKey
  if ((cert !== undefined) ^ (key !== undefined)) {
    throw new TypeError("If `cert` is specified, `key` must also be specified");
  }
  if ((certFile !== undefined) ^ (keyFile !== undefined)) {
    throw new TypeError(
      "If `certFile` is specified, `keyFile` must also be specified",
    );
  }
  if ((certChain !== undefined) ^ (privateKey !== undefined)) {
    throw new TypeError(
      "If `certChain` is specified, `privateKey` must also be specified",
    );
  }

  // Ensure that only one pair is valid
  if (certChain !== undefined && cert !== undefined) {
    throw new TypeError(
      "Cannot specify both `certChain` and `cert`",
    );
  }
  if (certChain !== undefined && certFile !== undefined) {
    throw new TypeError(
      "Cannot specify both `certChain` and `certFile`",
    );
  }

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
    return op_tls_key_static_from_file("Deno.listenTls", certChain, privateKey);
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
    throw new TypeError("A key and certificate are required for `listenTls`");
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
  } = {},
) {
  const { 0: rid, 1: localAddr, 2: remoteAddr } = op_tls_start({
    rid: conn[internalRidSymbol],
    hostname,
    caCerts,
    alpnProtocols,
  });
  return new TlsConn(rid, remoteAddr, localAddr);
}

export {
  connectTls,
  hasTlsKeyPairOptions,
  listenTls,
  loadTlsKeyPair,
  startTls,
  TlsConn,
  TlsListener,
};
