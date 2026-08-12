// Copyright 2018-2026 the Deno authors. MIT license.
import { core, internals, primordials } from "ext:core/mod.js";
import {
  op_quic_connecting_0rtt,
  op_quic_connecting_1rtt,
  op_quic_connection_accept_bi,
  op_quic_connection_accept_uni,
  op_quic_connection_close,
  op_quic_connection_closed,
  op_quic_connection_get_max_datagram_size,
  op_quic_connection_get_protocol,
  op_quic_connection_get_remote_addr,
  op_quic_connection_get_server_name,
  op_quic_connection_handshake,
  op_quic_connection_open_bi,
  op_quic_connection_open_uni,
  op_quic_connection_read_datagram,
  op_quic_connection_send_datagram,
  op_quic_endpoint_close,
  op_quic_endpoint_connect,
  op_quic_endpoint_create,
  op_quic_endpoint_get_addr,
  op_quic_endpoint_listen,
  op_quic_incoming_accept,
  op_quic_incoming_accept_0rtt,
  op_quic_incoming_ignore,
  op_quic_incoming_local_ip,
  op_quic_incoming_refuse,
  op_quic_incoming_remote_addr,
  op_quic_incoming_remote_addr_validated,
  op_quic_listener_accept,
  op_quic_listener_stop,
  op_quic_recv_stream_get_id,
  op_quic_send_stream_get_id,
  op_quic_send_stream_get_priority,
  op_quic_send_stream_set_priority,
  op_webtransport_accept,
  op_webtransport_connect,
} from "ext:core/ops";
const {
  getReadableStreamResourceBacking,
  getWritableStreamResourceBacking,
  ReadableStream,
  readableStreamForRid,
  WritableStream,
  writableStreamForRid,
} = core.loadExtScript("ext:deno_web/06_streams.js");
const { loadTlsKeyPair } = core.loadExtScript("ext:deno_net/02_tls.js");
const {
  BadResourcePrototype,
} = core;
const {
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
  ReflectConstruct,
  SafeSet,
  SetPrototypeAdd,
  SetPrototypeClear,
  SetPrototypeDelete,
  SetPrototypeForEach,
  SetPrototypeGetSize,
  Symbol,
  SymbolAsyncIterator,
} = primordials;

let getEndpointResource;

function promiseFinallyWithoutUnhandled(p, f) {
  return PromisePrototypeThen(p, f, f);
}

class QuicStreamResourceTracker {
  #closed = false;
  #resources = new SafeSet();

  constructor(closed) {
    // A connection owns all of its active stream resources. Keeping one
    // connection-close handler lets completed streams release that ownership.
    promiseFinallyWithoutUnhandled(closed, () => {
      this.#closed = true;
      SetPrototypeForEach(this.#resources, (rid) => core.tryClose(rid));
      SetPrototypeClear(this.#resources);
    });
  }

  track(rid) {
    if (this.#closed) {
      core.tryClose(rid);
      return () => {};
    }

    SetPrototypeAdd(this.#resources, rid);
    let resources = this.#resources;
    return () => {
      if (resources !== undefined) {
        SetPrototypeDelete(resources, rid);
        resources = undefined;
      }
    };
  }

  release(rid) {
    SetPrototypeDelete(this.#resources, rid);
    core.tryClose(rid);
  }

  get size() {
    return SetPrototypeGetSize(this.#resources);
  }
}

function transportOptions({
  keepAliveInterval,
  maxIdleTimeout,
  maxConcurrentBidirectionalStreams,
  maxConcurrentUnidirectionalStreams,
  preferredAddressV4,
  preferredAddressV6,
  congestionControl,
}) {
  return {
    keepAliveInterval,
    maxIdleTimeout,
    maxConcurrentBidirectionalStreams,
    maxConcurrentUnidirectionalStreams,
    preferredAddressV4,
    preferredAddressV6,
    congestionControl,
  };
}

const kRid = Symbol("rid");

class QuicEndpoint {
  #endpoint;

  constructor(
    { hostname = "::", port = 0, [kRid]: rid } = { __proto__: null },
  ) {
    this.#endpoint = rid ?? op_quic_endpoint_create({ hostname, port }, true);
  }

  get addr() {
    return op_quic_endpoint_get_addr(this.#endpoint);
  }

  listen(options) {
    const keyPair = loadTlsKeyPair("Deno.QuicEndpoint.listen", {
      cert: options.cert,
      key: options.key,
    });
    const listener = op_quic_endpoint_listen(
      this.#endpoint,
      { alpnProtocols: options.alpnProtocols },
      transportOptions(options),
      keyPair,
    );
    return new QuicListener(listener, this);
  }

  close({ closeCode = 0, reason = "" } = { __proto__: null }) {
    op_quic_endpoint_close(this.#endpoint, closeCode, reason);
  }

  static {
    getEndpointResource = (e) => e.#endpoint;
  }
}

class QuicListener {
  #listener;
  #endpoint;

  constructor(listener, endpoint) {
    this.#listener = listener;
    this.#endpoint = endpoint;
  }

  get endpoint() {
    return this.#endpoint;
  }

  async incoming() {
    const incoming = await op_quic_listener_accept(this.#listener);
    return new QuicIncoming(incoming, this.#endpoint);
  }

  async accept() {
    const incoming = await this.incoming();
    const connection = await incoming.accept();
    return connection;
  }

  async next() {
    try {
      const incoming = await this.incoming();
      return { value: incoming, done: false };
    } catch (error) {
      if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
        return { value: undefined, done: true };
      }
      throw error;
    }
  }

  [SymbolAsyncIterator]() {
    return this;
  }

  stop() {
    op_quic_listener_stop(this.#listener);
  }
}

class QuicIncoming {
  #incoming;
  #endpoint;

  constructor(incoming, endpoint) {
    this.#incoming = incoming;
    this.#endpoint = endpoint;
  }

  get localIp() {
    return op_quic_incoming_local_ip(this.#incoming);
  }

  get remoteAddr() {
    return op_quic_incoming_remote_addr(this.#incoming);
  }

  get remoteAddressValidated() {
    return op_quic_incoming_remote_addr_validated(this.#incoming);
  }

  accept(options) {
    const tOptions = options ? transportOptions(options) : null;
    if (options?.zeroRtt) {
      const conn = op_quic_incoming_accept_0rtt(
        this.#incoming,
        tOptions,
      );
      return new QuicConn(conn, this.#endpoint);
    }
    return PromisePrototypeThen(
      op_quic_incoming_accept(this.#incoming, tOptions),
      (conn) => new QuicConn(conn, this.#endpoint),
    );
  }

  refuse() {
    op_quic_incoming_refuse(this.#incoming);
  }

  ignore() {
    op_quic_incoming_ignore(this.#incoming);
  }
}

let webtransportConnect;
let webtransportAccept;
let getQuicStreamResourceCount;

class QuicConn {
  #resource;
  #bidiStream = null;
  #uniStream = null;
  #closed;
  #handshake;
  #endpoint;
  #streamResources;

  constructor(resource, endpoint) {
    this.#resource = resource;
    this.#endpoint = endpoint;

    this.#closed = op_quic_connection_closed(this.#resource);
    core.unrefOpPromise(this.#closed);
    this.#streamResources = new QuicStreamResourceTracker(this.#closed);
  }

  get endpoint() {
    return this.#endpoint;
  }

  get protocol() {
    return op_quic_connection_get_protocol(this.#resource);
  }

  get remoteAddr() {
    return op_quic_connection_get_remote_addr(this.#resource);
  }

  get serverName() {
    return op_quic_connection_get_server_name(this.#resource);
  }

  async createBidirectionalStream(
    { sendOrder, waitUntilAvailable } = { __proto__: null },
  ) {
    const { 0: txRid, 1: rxRid } = await op_quic_connection_open_bi(
      this.#resource,
      waitUntilAvailable ?? false,
    );
    try {
      if (sendOrder !== null && sendOrder !== undefined) {
        op_quic_send_stream_set_priority(txRid, sendOrder);
      }
      return new QuicBidirectionalStream(
        txRid,
        rxRid,
        this.#streamResources,
      );
    } catch (error) {
      this.#streamResources.release(txRid);
      this.#streamResources.release(rxRid);
      throw error;
    }
  }

  async createUnidirectionalStream(
    { sendOrder, waitUntilAvailable } = { __proto__: null },
  ) {
    const rid = await op_quic_connection_open_uni(
      this.#resource,
      waitUntilAvailable ?? false,
    );
    try {
      if (sendOrder !== null && sendOrder !== undefined) {
        op_quic_send_stream_set_priority(rid, sendOrder);
      }
      return writableStream(rid, this.#streamResources);
    } catch (error) {
      this.#streamResources.release(rid);
      throw error;
    }
  }

  get incomingBidirectionalStreams() {
    if (this.#bidiStream === null) {
      this.#bidiStream = ReadableStream.from(
        bidiStream(this.#resource, this.#streamResources),
      );
    }
    return this.#bidiStream;
  }

  get incomingUnidirectionalStreams() {
    if (this.#uniStream === null) {
      this.#uniStream = ReadableStream.from(
        uniStream(this.#resource, this.#streamResources),
      );
    }
    return this.#uniStream;
  }

  get maxDatagramSize() {
    return op_quic_connection_get_max_datagram_size(this.#resource);
  }

  async readDatagram() {
    const buffer = await op_quic_connection_read_datagram(this.#resource);
    return buffer;
  }

  async sendDatagram(data) {
    await op_quic_connection_send_datagram(this.#resource, data);
  }

  get handshake() {
    if (!this.#handshake) {
      this.#handshake = op_quic_connection_handshake(this.#resource);
    }
    return this.#handshake;
  }

  get closed() {
    core.refOpPromise(this.#closed);
    return this.#closed;
  }

  close({ closeCode = 0, reason = "" } = { __proto__: null }) {
    op_quic_connection_close(this.#resource, closeCode, reason);
  }

  static {
    getQuicStreamResourceCount = (conn) => conn.#streamResources.size;

    webtransportConnect = async function webtransportConnect(conn, url) {
      const {
        0: connectTxRid,
        1: connectRxRid,
        2: settingsTxRid,
        3: settingsRxRid,
      } = await op_webtransport_connect(conn.#resource, url);
      try {
        const connect = new QuicBidirectionalStream(
          connectTxRid,
          connectRxRid,
          conn.#streamResources,
        );
        const settingsTx = writableStream(
          settingsTxRid,
          conn.#streamResources,
        );
        const settingsRx = readableStream(
          settingsRxRid,
          conn.#streamResources,
        );
        return { connect, settingsTx, settingsRx };
      } catch (error) {
        conn.#streamResources.release(connectTxRid);
        conn.#streamResources.release(connectRxRid);
        conn.#streamResources.release(settingsTxRid);
        conn.#streamResources.release(settingsRxRid);
        throw error;
      }
    };

    webtransportAccept = async function webtransportAccept(conn) {
      const {
        0: url,
        1: connectTxRid,
        2: connectRxRid,
        3: settingsTxRid,
        4: settingsRxRid,
      } = await op_webtransport_accept(conn.#resource);
      try {
        const connect = new QuicBidirectionalStream(
          connectTxRid,
          connectRxRid,
          conn.#streamResources,
        );
        const settingsTx = writableStream(
          settingsTxRid,
          conn.#streamResources,
        );
        const settingsRx = readableStream(
          settingsRxRid,
          conn.#streamResources,
        );
        return { url, connect, settingsTx, settingsRx };
      } catch (error) {
        conn.#streamResources.release(connectTxRid);
        conn.#streamResources.release(connectRxRid);
        conn.#streamResources.release(settingsTxRid);
        conn.#streamResources.release(settingsRxRid);
        throw error;
      }
    };
  }
}

class QuicSendStream extends WritableStream {
  get sendOrder() {
    return op_quic_send_stream_get_priority(
      getWritableStreamResourceBacking(this).rid,
    );
  }

  set sendOrder(p) {
    op_quic_send_stream_set_priority(
      getWritableStreamResourceBacking(this).rid,
      p,
    );
  }

  get id() {
    return op_quic_send_stream_get_id(
      getWritableStreamResourceBacking(this).rid,
    );
  }
}

class QuicReceiveStream extends ReadableStream {
  get id() {
    return op_quic_recv_stream_get_id(
      getReadableStreamResourceBacking(this).rid,
    );
  }
}

function readableStream(rid, resources) {
  const onClose = resources.track(rid);
  try {
    return readableStreamForRid(
      rid,
      true,
      (...args) => ReflectConstruct(QuicReceiveStream, args),
      undefined,
      onClose,
    );
  } catch (error) {
    resources.release(rid);
    throw error;
  }
}

function writableStream(rid, resources) {
  const onClose = resources.track(rid);
  try {
    return writableStreamForRid(
      rid,
      true,
      (...args) => ReflectConstruct(QuicSendStream, args),
      { __proto__: null, onClose },
    );
  } catch (error) {
    resources.release(rid);
    throw error;
  }
}

class QuicBidirectionalStream {
  #readable;
  #writable;

  constructor(txRid, rxRid, resources) {
    try {
      this.#readable = readableStream(rxRid, resources);
      this.#writable = writableStream(txRid, resources);
    } catch (error) {
      resources.release(txRid);
      resources.release(rxRid);
      throw error;
    }
  }

  get readable() {
    return this.#readable;
  }

  get writable() {
    return this.#writable;
  }
}

async function* bidiStream(conn, resources) {
  try {
    while (true) {
      const r = await op_quic_connection_accept_bi(conn);
      yield new QuicBidirectionalStream(r[0], r[1], resources);
    }
  } catch (error) {
    if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
      return;
    }
    throw error;
  }
}

async function* uniStream(conn, resources) {
  try {
    while (true) {
      const uniRid = await op_quic_connection_accept_uni(conn);
      yield readableStream(uniRid, resources);
    }
  } catch (error) {
    if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
      return;
    }
    throw error;
  }
}

function connectQuic(options) {
  const endpoint = options.endpoint ??
    new QuicEndpoint({
      [kRid]: op_quic_endpoint_create({ hostname: "::", port: 0 }, 0, false),
    });
  const keyPair = loadTlsKeyPair("Deno.connectQuic", {
    cert: options.cert,
    key: options.key,
  });
  const connecting = op_quic_endpoint_connect(
    getEndpointResource(endpoint),
    {
      addr: {
        hostname: options.hostname,
        port: options.port,
      },
      caCerts: options.caCerts,
      alpnProtocols: options.alpnProtocols,
      serverName: options.serverName,
      serverCertificateHashes: options.serverCertificateHashes,
    },
    transportOptions(options),
    keyPair,
  );

  if (options.zeroRtt) {
    const conn = op_quic_connecting_0rtt(connecting);
    if (conn) {
      return new QuicConn(conn, endpoint);
    }
  }

  return PromisePrototypeThen(
    op_quic_connecting_1rtt(connecting),
    (conn) => new QuicConn(conn, endpoint),
  );
}

internals.getQuicStreamResourceCount = getQuicStreamResourceCount;

export {
  connectQuic,
  QuicBidirectionalStream,
  QuicConn,
  QuicEndpoint,
  QuicIncoming,
  QuicListener,
  QuicReceiveStream,
  QuicSendStream,
  webtransportAccept,
  webtransportConnect,
};
