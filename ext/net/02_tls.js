// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, internals, primordials } = __bootstrap;
const { internalRidSymbol } = core;
const {
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
  op_tls_peer_certificate,
  op_tls_start,
} = core.ops;
const {
  ObjectDefineProperty,
  ObjectFreeze,
  TypeError,
  Symbol,
  SymbolFor,
} = primordials;

const { Conn, Listener, validatePort } = core.loadExtScript(
  "ext:deno_net/01_net.js",
);

const _getPeerCertificate = Symbol("getPeerCertificate");

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

  [_getPeerCertificate](detailed = false) {
    return op_tls_peer_certificate(this.#rid, detailed);
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
  unsafelyDisableHostnameVerification = false,
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
    { caCerts, alpnProtocols, serverName, unsafelyDisableHostnameVerification },
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
  return (options.cert !== undefined || options.key !== undefined ||
    options.resolveCertificate !== undefined);
}

/**
 * Loads a TLS keypair from one of the various options. If no key material is provided,
 * returns a special Null keypair.
 */
function loadTlsKeyPair(api, {
  keyFormat,
  cert,
  key,
  resolveCertificate,
}) {
  if (resolveCertificate !== undefined) {
    if (typeof resolveCertificate !== "function") {
      throw new TypeError(
        `If \`resolveCertificate\` is specified, it must be a function for \`${api}\``,
      );
    }
    return createTlsKeyResolver(resolveCertificate);
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
  port = 0,
  hostname = "0.0.0.0",
  transport = "tcp",
  alpnProtocols = undefined,
  reusePort = false,
  tcpBacklog = 511,
}) {
  if (transport !== "tcp") {
    throw new TypeError(`Unsupported transport: '${transport}'`);
  }
  port = validatePort(port, true);

  if (!hasTlsKeyPairOptions(arguments[0])) {
    throw new TypeError(
      "A key and certificate are required for `Deno.listenTls`",
    );
  }
  const keyPair = loadTlsKeyPair("Deno.listenTls", arguments[0]);
  const { 0: rid, 1: localAddr } = op_net_listen_tls(
    { hostname, port },
    { alpnProtocols, reusePort, tcpBacklog },
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
    unsafelyDisableHostnameVerification = false,
  } = { __proto__: null },
) {
  return startTlsInternal(conn, {
    hostname,
    caCerts,
    alpnProtocols,
    unsafelyDisableHostnameVerification,
  });
}

function startTlsInternal(
  conn,
  {
    hostname = "127.0.0.1",
    caCerts = [],
    alpnProtocols = undefined,
    keyPair = null,
    rejectUnauthorized,
    unsafelyDisableHostnameVerification,
  },
) {
  const { 0: rid, 1: localAddr, 2: remoteAddr } = op_tls_start({
    rid: conn[internalRidSymbol],
    hostname,
    caCerts,
    alpnProtocols,
    rejectUnauthorized,
    unsafelyDisableHostnameVerification,
  }, keyPair);
  return new TlsConn(rid, remoteAddr, localAddr);
}

const serverNameSymbol = SymbolFor("unstableServerName");

// Drives a `resolveCertificate` callback: polls the native lookup queue for
// pending TLS handshakes, invokes the user callback with the requested server
// name (SNI) and the client's ClientHello, and feeds the resulting
// certificate/key pair back. Resolutions are cached by server name on the
// native side, so the callback is invoked once per distinct name.
function createTlsKeyResolver(callback) {
  const { 0: resolver, 1: lookup } = op_tls_cert_resolver_create();
  (async () => {
    while (true) {
      const polled = await op_tls_cert_resolver_poll(lookup);
      if (polled === null || polled === undefined) {
        break;
      }
      const { 0: id, 1: serverName, 2: clientHello } = polled;
      try {
        const keyPair = await callback(serverName, ObjectFreeze(clientHello));
        if (
          keyPair === null || typeof keyPair !== "object" ||
          keyPair.cert === undefined || keyPair.key === undefined
        ) {
          op_tls_cert_resolver_resolve_error(
            lookup,
            id,
            "`resolveCertificate` must return an object with `cert` and `key`",
          );
        } else {
          const resolved = loadTlsKeyPair("Deno.listenTls", keyPair);
          op_tls_cert_resolver_resolve(lookup, id, resolved);
        }
      } catch (e) {
        op_tls_cert_resolver_resolve_error(lookup, id, e.message);
      }
    }
  })();
  return resolver;
}

internals.serverNameSymbol = serverNameSymbol;
internals.getPeerCertificate = _getPeerCertificate;

return {
  connectTls,
  hasTlsKeyPairOptions,
  listenTls,
  loadTlsKeyPair,
  startTls,
  startTlsInternal,
  TlsConn,
  TlsListener,
};
})();
