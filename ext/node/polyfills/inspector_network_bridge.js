// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file prefer-primordials

// Installs `internals.__inspectorNetwork`, the bridge that other
// extensions (ext/fetch, http, websocket) use to emit `Network.*` CDP
// events without depending on ext/node directly. It needs to be
// installed eagerly so that `deno run --inspect` alone is enough to
// see fetch traffic in DevTools - previously the bridge lived in the
// `node:inspector` polyfill and only appeared if user code imported
// that module.

(function () {
const { core, internals, primordials } = __bootstrap;
const {
  op_base64_encode_from_buffer,
  op_inspector_emit_protocol_event,
  op_inspector_enabled,
} = core.ops;
const {
  JSONStringify,
  ObjectAssign,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetSymbolToStringTag,
  Uint8Array,
} = primordials;

function encodeNetworkData(data) {
  if (data == null) return undefined;
  if (typeof data === "string") {
    const buf = core.encode(data);
    return op_base64_encode_from_buffer(buf, 0, buf.byteLength);
  }
  if (TypedArrayPrototypeGetSymbolToStringTag(data) === "Uint8Array") {
    return op_base64_encode_from_buffer(
      data,
      0,
      TypedArrayPrototypeGetByteLength(data),
    );
  }
  if (data instanceof ArrayBuffer) {
    const view = new Uint8Array(data);
    return op_base64_encode_from_buffer(view, 0, view.byteLength);
  }
  throw new TypeError(
    "Expected data to be a string, Buffer, Uint8Array, or ArrayBuffer",
  );
}

function emit(eventName, params) {
  op_inspector_emit_protocol_event(eventName, JSONStringify(params ?? {}));
}

function emitWithData(eventName, params) {
  if (params && params.data !== undefined) {
    const encoded = encodeNetworkData(params.data);
    if (encoded !== params.data) {
      params = ObjectAssign({ __proto__: null }, params, { data: encoded });
    }
  }
  emit(eventName, params);
}

// CDP's `webSocketFrameSent`/`webSocketFrameReceived` carry the payload
// inside `params.response.payloadData`. Text frames go through as-is, binary
// frames must be base64-encoded (DevTools uses `opcode` to decide how to
// render the panel - 1=text, 2=binary).
function emitFrame(eventName, params) {
  const payload = params?.response?.payloadData;
  if (payload != null && typeof payload !== "string") {
    let view = null;
    if (TypedArrayPrototypeGetSymbolToStringTag(payload) === "Uint8Array") {
      view = payload;
    } else if (payload instanceof ArrayBuffer) {
      view = new Uint8Array(payload);
    }
    if (view !== null) {
      const encoded = op_base64_encode_from_buffer(
        view,
        0,
        TypedArrayPrototypeGetByteLength(view),
      );
      params = ObjectAssign({ __proto__: null }, params, {
        response: ObjectAssign({ __proto__: null }, params.response, {
          payloadData: encoded,
        }),
      });
    }
  }
  emit(eventName, params);
}

let networkRequestIdCounter = 0;
internals.__inspectorNetwork = {
  isEnabled: () => op_inspector_enabled(),
  nextRequestId: () => `node-network-event-${++networkRequestIdCounter}`,
  requestWillBeSent: (p) => emit("Network.requestWillBeSent", p),
  responseReceived: (p) => emit("Network.responseReceived", p),
  loadingFinished: (p) => emit("Network.loadingFinished", p),
  loadingFailed: (p) => emit("Network.loadingFailed", p),
  dataReceived: (p) => emitWithData("Network.dataReceived", p),
  dataSent: (p) => emitWithData("Network.dataSent", p),
  webSocketCreated: (p) => emit("Network.webSocketCreated", p),
  // Not part of the public `inspector.Network` surface (Node doesn't
  // expose it either - see node_compat test-inspector-emit-protocol-event)
  // but DevTools still needs the event to populate the request-side
  // Headers panel for websocket connections.
  webSocketWillSendHandshakeRequest: (p) =>
    emit("Network.webSocketWillSendHandshakeRequest", p),
  webSocketHandshakeResponseReceived: (p) =>
    emit("Network.webSocketHandshakeResponseReceived", p),
  webSocketClosed: (p) => emit("Network.webSocketClosed", p),
  webSocketFrameSent: (p) => emitFrame("Network.webSocketFrameSent", p),
  webSocketFrameReceived: (p) => emitFrame("Network.webSocketFrameReceived", p),
  webSocketFrameError: (p) => emit("Network.webSocketFrameError", p),
};
})();
