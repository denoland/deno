// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { core, primordials } from "ext:core/mod.js";
import {
  op_quic_accept,
  op_quic_accept_bi,
  op_quic_accept_incoming,
  op_quic_accept_uni,
  op_quic_close_connection,
  op_quic_close_endpoint,
  op_quic_connect,
  op_quic_connection_closed,
  op_quic_connection_get_protocol,
  op_quic_connection_get_remote_addr,
  op_quic_endpoint_get_addr,
  op_quic_get_send_stream_priority,
  op_quic_incoming_accept,
  op_quic_incoming_ignore,
  op_quic_incoming_refuse,
  op_quic_listen,
  op_quic_max_datagram_size,
  op_quic_open_bi,
  op_quic_open_uni,
  op_quic_read_datagram,
  op_quic_send_datagram,
  op_quic_set_send_stream_priority,
} from "ext:core/ops";
import {
  getWritableStreamResourceBacking,
  ReadableStream,
  readableStreamForRid,
  WritableStream,
  writableStreamForRid,
} from "ext:deno_web/06_streams.js";
import { loadTlsKeyPair } from "ext:deno_net/02_tls.js";
const {
  BadResourcePrototype,
} = core;
const {
  Uint8Array,
  TypedArrayPrototypeSubarray,
  SymbolAsyncIterator,
  SafePromisePrototypeFinally,
  ObjectPrototypeIsPrototypeOf,
} = primordials;

class QuicSendStream extends WritableStream {
  get sendOrder() {
    return op_quic_get_send_stream_priority(
      getWritableStreamResourceBacking(this).rid,
    );
  }

  set sendOrder(p) {
    op_quic_set_send_stream_priority(
      getWritableStreamResourceBacking(this).rid,
      p,
    );
  }
}

class QuicReceiveStream extends ReadableStream {}

function readableStream(rid, closed) {
  // stream can be indirectly closed by closing connection.
  SafePromisePrototypeFinally(closed, () => {
    core.tryClose(rid);
  });
  return readableStreamForRid(rid, true, QuicReceiveStream);
}

function writableStream(rid, closed) {
  // stream can be indirectly closed by closing connection.
  SafePromisePrototypeFinally(closed, () => {
    core.tryClose(rid);
  });
  return writableStreamForRid(rid, true, QuicSendStream);
}

class QuicBidirectionalStream {
  #readable;
  #writable;

  constructor(txRid, rxRid, closed) {
    this.#readable = readableStream(rxRid, closed);
    this.#writable = writableStream(txRid, closed);
  }

  get readable() {
    return this.#readable;
  }

  get writable() {
    return this.#writable;
  }
}

async function* bidiStream(conn, closed) {
  try {
    while (true) {
      const r = await op_quic_accept_bi(conn);
      yield new QuicBidirectionalStream(r[0], r[1], closed);
    }
  } catch (error) {
    if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
      return;
    }
    throw error;
  }
}

async function* uniStream(conn, closed) {
  try {
    while (true) {
      const uniRid = await op_quic_accept_uni(conn);
      yield readableStream(uniRid, closed);
    }
  } catch (error) {
    if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
      return;
    }
    throw error;
  }
}

class QuicConn {
  #resource;
  #bidiStream = null;
  #uniStream = null;
  #closed;

  constructor(resource) {
    this.#resource = resource;

    this.#closed = op_quic_connection_closed(this.#resource);
    core.unrefOpPromise(this.#closed);
  }

  get protocol() {
    return op_quic_connection_get_protocol(this.#resource);
  }

  get remoteAddr() {
    return op_quic_connection_get_remote_addr(this.#resource);
  }

  async createBidirectionalStream({ sendOrder, waitUntilAvailable } = {}) {
    const { 0: txRid, 1: rxRid } = await op_quic_open_bi(
      this.#resource,
      waitUntilAvailable ?? false,
    );
    if (sendOrder !== null && sendOrder !== undefined) {
      op_quic_set_send_stream_priority(txRid, sendOrder);
    }
    return new QuicBidirectionalStream(txRid, rxRid, this.#closed);
  }

  async createUnidirectionalStream({ sendOrder, waitUntilAvailable } = {}) {
    const rid = await op_quic_open_uni(
      this.#resource,
      waitUntilAvailable ?? false,
    );
    if (sendOrder !== null && sendOrder !== undefined) {
      op_quic_set_send_stream_priority(rid, sendOrder);
    }
    return writableStream(rid, this.#closed);
  }

  get incomingBidirectionalStreams() {
    if (this.#bidiStream === null) {
      this.#bidiStream = ReadableStream.from(
        bidiStream(this.#resource, this.#closed),
      );
    }
    return this.#bidiStream;
  }

  get incomingUnidirectionalStreams() {
    if (this.#uniStream === null) {
      this.#uniStream = ReadableStream.from(
        uniStream(this.#resource, this.#closed),
      );
    }
    return this.#uniStream;
  }

  get maxDatagramSize() {
    return op_quic_max_datagram_size(this.#resource);
  }

  async readDatagram(p) {
    const view = p || new Uint8Array(this.maxDatagramSize);
    const nread = await op_quic_read_datagram(this.#resource, view);
    return TypedArrayPrototypeSubarray(view, 0, nread);
  }

  async sendDatagram(data) {
    await op_quic_send_datagram(this.#resource, data);
  }

  get closed() {
    core.refOpPromise(this.#closed);
    return this.#closed;
  }

  close({ closeCode, reason }) {
    op_quic_close_connection(this.#resource, closeCode, reason);
  }
}

class QuicIncoming {
  #incoming;

  constructor(incoming) {
    this.#incoming = incoming;
  }

  async accept() {
    const conn = await op_quic_incoming_accept(this.#incoming);
    return new QuicConn(conn);
  }

  async refuse() {
    await op_quic_incoming_refuse(this.#incoming);
  }

  async ignore() {
    await op_quic_incoming_ignore(this.#incoming);
  }
}

class QuicListener {
  #endpoint;

  constructor(endpoint) {
    this.#endpoint = endpoint;
  }

  get addr() {
    return op_quic_endpoint_get_addr(this.#endpoint);
  }

  async accept() {
    const conn = await op_quic_accept(this.#endpoint);
    return new QuicConn(conn);
  }

  async incoming() {
    const incoming = await op_quic_accept_incoming(this.#endpoint);
    return new QuicIncoming(incoming);
  }

  async next() {
    let conn;
    try {
      conn = await this.accept();
    } catch (error) {
      if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, error)) {
        return { value: undefined, done: true };
      }
      throw error;
    }
    return { value: conn, done: false };
  }

  [SymbolAsyncIterator]() {
    return this;
  }

  close({ closeCode, reason }) {
    op_quic_close_endpoint(this.#endpoint, closeCode, reason);
  }
}

async function listenQuic(
  {
    hostname,
    port,
    cert,
    key,
    alpnProtocols,
    keepAliveInterval,
    maxIdleTimeout,
    maxConcurrentBidirectionalStreams,
    maxConcurrentUnidirectionalStreams,
  },
) {
  hostname = hostname || "0.0.0.0";
  const keyPair = loadTlsKeyPair("Deno.listenQuic", { cert, key });
  const endpoint = await op_quic_listen(
    { hostname, port },
    { alpnProtocols },
    {
      keepAliveInterval,
      maxIdleTimeout,
      maxConcurrentBidirectionalStreams,
      maxConcurrentUnidirectionalStreams,
    },
    keyPair,
  );
  return new QuicListener(endpoint);
}

async function connectQuic(
  {
    hostname,
    port,
    serverName,
    caCerts,
    cert,
    key,
    alpnProtocols,
    keepAliveInterval,
    maxIdleTimeout,
    maxConcurrentBidirectionalStreams,
    maxConcurrentUnidirectionalStreams,
  },
) {
  const keyPair = loadTlsKeyPair("Deno.connectQuic", { cert, key });
  const conn = await op_quic_connect(
    { hostname, port },
    {
      caCerts,
      alpnProtocols,
      serverName,
    },
    {
      keepAliveInterval,
      maxIdleTimeout,
      maxConcurrentBidirectionalStreams,
      maxConcurrentUnidirectionalStreams,
    },
    keyPair,
  );
  return new QuicConn(conn);
}

export {
  connectQuic,
  listenQuic,
  QuicBidirectionalStream,
  QuicConn,
  QuicIncoming,
  QuicListener,
  QuicReceiveStream,
  QuicSendStream,
};
