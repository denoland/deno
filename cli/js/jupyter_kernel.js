// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file

/**
 * Jupyter ZMQ kernel implemented entirely in JS using Deno.listen().
 *
 * Architecture:
 *   Main OS thread   - this file; runs the ZMTP/Jupyter protocol (non-blocking TCP I/O)
 *   Background thread - REPL session that evaluates user code
 *
 * The two threads communicate via Rust ops backed by mpsc channels.
 */

import { core, internals } from "ext:core/mod.js";
const {
  op_jupyter_get_connection_info,
  op_jupyter_repl_evaluate,
  op_jupyter_repl_get_properties,
  op_jupyter_repl_global_lexical_scope_names,
  op_jupyter_repl_call_function_on_args,
  op_jupyter_repl_call_function_on,
  op_jupyter_repl_interrupt,
  op_jupyter_repl_cancel_interrupt,
  op_jupyter_recv_iopub,
  op_jupyter_recv_input,
  op_jupyter_send_input_reply,
  op_jupyter_deno_version,
  op_jupyter_typescript_version,
} = core.ops;

// --- ZMTP frame helpers --------------------------------------------------------

const ENC = new TextEncoder();
const DEC = new TextDecoder();

async function readExact(conn, n) {
  const buf = new Uint8Array(n);
  let offset = 0;
  while (offset < n) {
    const nread = await conn.read(buf.subarray(offset));
    if (nread === null) throw new Error("connection closed");
    offset += nread;
  }
  return buf;
}

async function readFrame(conn) {
  const flagBuf = await readExact(conn, 1);
  const flag = flagBuf[0];
  const isCommand = (flag & 0x04) !== 0;
  const isLong = (flag & 0x02) !== 0;
  const hasMore = (flag & 0x01) !== 0;

  let size;
  if (isLong) {
    const b = await readExact(conn, 8);
    const dv = new DataView(b.buffer);
    // upper 4 bytes should be 0 for sane message sizes
    size = dv.getUint32(4);
  } else {
    const b = await readExact(conn, 1);
    size = b[0];
  }

  const data = await readExact(conn, size);
  return { data, hasMore, isCommand };
}

function makeShortFrame(data, more, isCommand = false) {
  const flag = (more ? 0x01 : 0x00) |
    (data.length > 255 ? 0x02 : 0x00) |
    (isCommand ? 0x04 : 0x00);
  if (data.length > 255) {
    const buf = new Uint8Array(9 + data.length);
    buf[0] = flag;
    new DataView(buf.buffer).setUint32(5, data.length);
    buf.set(data, 9);
    return buf;
  }
  const buf = new Uint8Array(2 + data.length);
  buf[0] = flag;
  buf[1] = data.length;
  buf.set(data, 2);
  return buf;
}

async function writeAll(conn, buf) {
  let offset = 0;
  while (offset < buf.length) {
    offset += await conn.write(buf.subarray(offset));
  }
}

async function sendFrames(conn, frames) {
  // Coalesce all ZMTP frames into a single write. libzmq peers
  // (VSCode/JupyterLab) send each message as one write; emitting many small
  // writes interacts badly with Nagle/delayed-ACK and can stall delivery.
  const encoded = frames.map((frame, i) =>
    makeShortFrame(frame, i < frames.length - 1)
  );
  const total = encoded.reduce((acc, f) => acc + f.length, 0);
  const buf = new Uint8Array(total);
  let offset = 0;
  for (const f of encoded) {
    buf.set(f, offset);
    offset += f.length;
  }
  await writeAll(conn, buf);
}

// --- ZMTP 3.0 handshake --------------------------------------------------------

function makeGreeting() {
  // 64-octet ZMTP greeting: signature (0xff, 8 zero pad, 0x7f), version 3.0,
  // 20-octet mechanism ("NULL"), as-server flag, 31 filler octets. The NULL
  // mechanism is symmetric and libzmq (VSCode/JupyterLab) sends as-server=0 with
  // an all-zero signature pad; a binding peer that sets as-server=1 or a nonzero
  // pad byte leaves a real libzmq peer in a handshake state where it never
  // delivers our messages. Mirror what the previous libzmq-based kernel sent.
  // Regression from #34083 (the JS kernel rewrite).
  const buf = new Uint8Array(64);
  buf[0] = 0xff;
  buf[9] = 0x7f;
  buf[10] = 0x03; // version major
  buf[11] = 0x00; // version minor
  buf.set(ENC.encode("NULL"), 12); // mechanism, null-padded to 20 octets
  buf[32] = 0x00; // as-server
  return buf;
}

function makeReadyCommand(socketType) {
  const sockBytes = ENC.encode(socketType);
  const nameBytes = ENC.encode("READY");
  const propName = ENC.encode("Socket-Type");

  // ZMTP command body: <nameLen:1><name><metadata...>. The leading command-name
  // length octet is mandatory; without it libzmq (VSCode/JupyterLab) parses the
  // first body byte ('R'=0x52=82) as the name length, fails to parse the
  // command, and tears down the connection so the handshake never completes.
  // Regression from #34083 (the JS kernel rewrite).
  // property encoding: <len1:propNameLen><propName><len4:valueLen><value>
  const propLen = 1 + propName.length + 4 + sockBytes.length;
  const body = new Uint8Array(1 + nameBytes.length + propLen);
  let o = 0;
  body[o++] = nameBytes.length;
  body.set(nameBytes, o);
  o += nameBytes.length;
  body[o++] = propName.length;
  body.set(propName, o);
  o += propName.length;
  new DataView(body.buffer).setUint32(o, sockBytes.length);
  o += 4;
  body.set(sockBytes, o);

  return makeShortFrame(body, false, true); // command flag
}

async function zmtpHandshake(conn, socketType) {
  await writeAll(conn, makeGreeting());
  // Read the peer's 64-octet greeting.
  await readExact(conn, 64);
  // NULL security handshake: exchange READY commands.
  await writeAll(conn, makeReadyCommand(socketType));
  await readFrame(conn);
}

// --- Jupyter wire protocol -----------------------------------------------------

const DELIMITER = ENC.encode("<IDS|MSG>");

async function hmacSign(key, parts) {
  if (!key || key.length === 0) return "";
  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    ENC.encode(key),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const combined = new Uint8Array(
    parts.reduce((acc, p) => acc + p.length, 0),
  );
  let offset = 0;
  for (const p of parts) {
    combined.set(p, offset);
    offset += p.length;
  }
  const sig = await crypto.subtle.sign("HMAC", cryptoKey, combined);
  return Array.from(new Uint8Array(sig))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

// Verifies the HMAC-SHA256 signature a peer sent against the signed frames.
// Returns true when the signature is valid, or when no key is configured
// (matching `hmacSign`, which emits an empty signature in that case).
//
// Incoming messages that fail this check must be dropped: the kernel runs with
// full permissions, so without signature verification any local process able
// to reach the kernel's (loopback) TCP ports could inject an `execute_request`
// and run arbitrary code. The Jupyter wire protocol requires this check.
async function hmacVerify(key, parts, sig) {
  if (!key || key.length === 0) return true;
  // The signature travels as a lowercase hex string; decode it to bytes.
  if (typeof sig !== "string" || sig.length === 0 || sig.length % 2 !== 0) {
    return false;
  }
  const sigBytes = new Uint8Array(sig.length / 2);
  for (let i = 0; i < sigBytes.length; i++) {
    const byte = Number.parseInt(sig.slice(i * 2, i * 2 + 2), 16);
    if (Number.isNaN(byte)) return false;
    sigBytes[i] = byte;
  }
  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    ENC.encode(key),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["verify"],
  );
  const combined = new Uint8Array(
    parts.reduce((acc, p) => acc + p.length, 0),
  );
  let offset = 0;
  for (const p of parts) {
    combined.set(p, offset);
    offset += p.length;
  }
  // crypto.subtle.verify performs the comparison without leaking timing.
  return await crypto.subtle.verify("HMAC", cryptoKey, sigBytes, combined);
}

function makeHeader(session, msgType) {
  return JSON.stringify({
    msg_id: crypto.randomUUID(),
    session,
    date: new Date().toISOString(),
    username: "kernel",
    msg_type: msgType,
    version: "5.3",
  });
}

async function encodeMsg(
  session,
  key,
  identities,
  msgType,
  content,
  parentHeader = {},
  metadata = {},
  buffers = [],
) {
  const header = makeHeader(session, msgType);
  const parentHeaderStr = JSON.stringify(parentHeader);
  const metaStr = JSON.stringify(metadata);
  const contentStr = JSON.stringify(content);

  const parts = [header, parentHeaderStr, metaStr, contentStr].map((s) =>
    ENC.encode(s)
  );
  const sig = await hmacSign(key, parts);

  // frames: [identities..., DELIMITER, sig, header, parent_header, metadata, content, buffers...]
  const frames = [
    ...identities,
    DELIMITER,
    ENC.encode(sig),
    ENC.encode(header),
    ENC.encode(parentHeaderStr),
    ENC.encode(metaStr),
    ENC.encode(contentStr),
    ...buffers.map((b) => b instanceof Uint8Array ? b : new Uint8Array(b)),
  ];
  return frames;
}

function decodeMsg(frames) {
  // find DELIMITER
  let delimIdx = -1;
  for (let i = 0; i < frames.length; i++) {
    if (frames[i].length === DELIMITER.length) {
      let match = true;
      for (let j = 0; j < DELIMITER.length; j++) {
        if (frames[i][j] !== DELIMITER[j]) {
          match = false;
          break;
        }
      }
      if (match) {
        delimIdx = i;
        break;
      }
    }
  }
  if (delimIdx === -1) throw new Error("no <IDS|MSG> delimiter");

  const identities = frames.slice(0, delimIdx);
  const sig = DEC.decode(frames[delimIdx + 1]);
  // The raw bytes of the four frames the signature is computed over. Kept so
  // the signature can be verified against exactly what was received, rather
  // than a re-serialization that might differ byte-for-byte.
  const signedParts = frames.slice(delimIdx + 2, delimIdx + 6);
  const header = JSON.parse(DEC.decode(frames[delimIdx + 2]));
  const parentHeader = JSON.parse(DEC.decode(frames[delimIdx + 3]));
  const metadata = JSON.parse(DEC.decode(frames[delimIdx + 4]));
  const content = JSON.parse(DEC.decode(frames[delimIdx + 5]));
  const buffers = frames.slice(delimIdx + 6);

  return {
    identities,
    sig,
    signedParts,
    header,
    parentHeader,
    metadata,
    content,
    buffers,
  };
}

// --- ZMTP socket helpers -------------------------------------------------------

async function recvMultipart(conn) {
  const frames = [];
  while (true) {
    const { data, hasMore } = await readFrame(conn);
    frames.push(data);
    if (!hasMore) break;
  }
  return frames;
}

// --- Per-channel servers -------------------------------------------------------

/**
 * REP socket server (heartbeat).
 * For each connected peer, echo back every received message.
 */
function listenOptions(info, port) {
  switch (info.transport ?? "tcp") {
    case "tcp":
      return { hostname: info.ip, port };
    case "ipc":
      return { transport: "unix", path: `${info.ip}-${port}` };
    default:
      throw new TypeError(`Unsupported Jupyter transport: ${info.transport}`);
  }
}

async function runHeartbeat(info, port) {
  const listener = Deno.listen(listenOptions(info, port));
  while (true) {
    const conn = await listener.accept();
    (async () => {
      try {
        await zmtpHandshake(conn, "REP");
        while (true) {
          const frames = await recvMultipart(conn);
          // Echo back
          await sendFrames(conn, frames);
        }
      } catch {
        // peer disconnected
      } finally {
        try {
          conn.close();
        } catch { /**/ }
      }
    })();
  }
}

/**
 * ROUTER socket server.
 * Returns an object with send/recv channels backed by async queues.
 */
function makeQueue() {
  const items = [];
  const waiters = [];
  return {
    push(item) {
      if (waiters.length > 0) {
        waiters.shift()(item);
      } else {
        items.push(item);
      }
    },
    async pop() {
      if (items.length > 0) return items.shift();
      return new Promise((resolve) => waiters.push(resolve));
    },
  };
}

/**
 * Per-connection ROUTER: each accepted connection maps to a peer with a
 * generated identity. Incoming messages are queued with the peer id prepended;
 * outgoing messages are dispatched to a specific peer by id (or broadcast to
 * every connected peer via `sendAll`).
 */
class RouterSocket {
  constructor(info, port) {
    this.port = port;
    this.info = info;
    this.incoming = makeQueue();
    this.peers = new Map(); // peerId (string) -> conn
    this._listen();
  }

  _listen() {
    (async () => {
      const listener = Deno.listen(listenOptions(this.info, this.port));
      while (true) {
        const conn = await listener.accept();
        this._handlePeer(conn);
      }
    })();
  }

  _handlePeer(conn) {
    const peerId = crypto.getRandomValues(new Uint8Array(5));
    const peerKey = Array.from(peerId).join(",");
    this.peers.set(peerKey, conn);
    (async () => {
      try {
        await zmtpHandshake(conn, "ROUTER");
        while (true) {
          const frames = await recvMultipart(conn);
          this.incoming.push({ peerId, peerKey, frames });
        }
      } catch {
        // disconnected
      } finally {
        this.peers.delete(peerKey);
        try {
          conn.close();
        } catch { /**/ }
      }
    })();
  }

  async recv() {
    return await this.incoming.pop();
  }

  async send(peerId, frames) {
    const peerKey = Array.from(peerId).join(",");
    const conn = this.peers.get(peerKey);
    if (!conn) return;
    try {
      await sendFrames(conn, frames);
    } catch {
      // The peer disconnected between sending its request and us routing
      // the reply. Drop it so the shell/control loop stays alive; the peer
      // will resend after reconnecting. See denoland/deno#20542.
      this.peers.delete(peerKey);
      try {
        conn.close();
      } catch { /**/ }
    }
  }

  async sendAll(frames) {
    const dead = [];
    for (const [peerKey, conn] of this.peers) {
      try {
        await sendFrames(conn, frames);
      } catch {
        dead.push(peerKey);
      }
    }
    for (const peerKey of dead) this.peers.delete(peerKey);
  }
}

/**
 * PUB socket server.
 * Sends the same frames to all connected subscribers.
 */
class PubSocket {
  constructor(info, port) {
    this.port = port;
    this.info = info;
    this.conns = new Set();
    this._listen();
  }

  _listen() {
    (async () => {
      const listener = Deno.listen(listenOptions(this.info, this.port));
      while (true) {
        const conn = await listener.accept();
        (async () => {
          try {
            await zmtpHandshake(conn, "PUB");
            // SUB sockets send a SUBSCRIBE command; drain it
            const subFrame = await recvMultipart(conn);
            void subFrame;
            this.conns.add(conn);
            // Keep connection alive; SUB may re-subscribe
            while (true) {
              try {
                await recvMultipart(conn);
              } catch {
                break;
              }
            }
          } catch {
            // disconnected
          } finally {
            this.conns.delete(conn);
            try {
              conn.close();
            } catch { /**/ }
          }
        })();
      }
    })();
  }

  async send(frames) {
    const dead = [];
    for (const conn of this.conns) {
      try {
        await sendFrames(conn, frames);
      } catch {
        dead.push(conn);
      }
    }
    for (const conn of dead) this.conns.delete(conn);
  }
}

// --- Main kernel logic ----------------------------------------------------------

async function startJupyterKernel() {
  const info = JSON.parse(op_jupyter_get_connection_info());
  const { key, hb_port, shell_port, control_port, stdin_port, iopub_port } =
    info;
  const session = crypto.randomUUID();

  // Start heartbeat (purely async, fire-and-forget)
  runHeartbeat(info, hb_port);

  const shell = new RouterSocket(info, shell_port);
  const control = new RouterSocket(info, control_port);
  const iopub = new PubSocket(info, iopub_port);
  const stdin = new RouterSocket(info, stdin_port);

  let executionCount = 0;
  let currentParentHeader = {};
  let currentAllowStdin = false;
  let shuttingDown = false;

  async function publishStatus(status, parentHeader) {
    const frames = await encodeMsg(
      session,
      key,
      [],
      "status",
      { execution_state: status },
      parentHeader,
    );
    await iopub.send(frames);
  }

  async function publishIopub(msg) {
    // Determine actual msg_type for standard stream messages
    let msgType = msg.msg_type;
    if (msgType === "stream_stdout" || msgType === "stream_stderr") {
      msgType = "stream";
    }
    const frames = await encodeMsg(
      session,
      key,
      [],
      msgType,
      msg.content,
      currentParentHeader,
      msg.metadata || {},
      msg.buffers || [],
    );
    await iopub.send(frames);
  }

  // Drain iopub messages from the REPL thread continuously.
  async function iopubDrainer() {
    while (true) {
      const msg = await op_jupyter_recv_iopub();
      if (msg !== null && msg !== undefined) {
        try {
          await publishIopub(msg);
        } catch (e) {
          // ignore publish errors
          void e;
        }
      }
    }
  }
  iopubDrainer();

  function kernelInfo() {
    return {
      status: "ok",
      protocol_version: "5.3",
      implementation: "Deno kernel",
      implementation_version: op_jupyter_deno_version(),
      language_info: {
        name: "typescript",
        version: op_jupyter_typescript_version(),
        mimetype: "text/x.typescript",
        file_extension: ".ts",
        pygments_lexer: "typescript",
        codemirror_mode: { name: "typescript" },
        nbconvert_exporter: "script",
      },
      banner: "Welcome to Deno kernel",
      help_links: [{ text: "Visit Deno manual", url: "https://docs.deno.com" }],
      debugger: false,
    };
  }

  function checkIsComplete(code) {
    let stack = [];
    let i = 0;
    while (i < code.length) {
      const ch = code[i];
      if (ch === "/" && code[i + 1] === "/") {
        while (i < code.length && code[i] !== "\n") i++;
        continue;
      }
      if (ch === "/" && code[i + 1] === "*") {
        i += 2;
        let closed = false;
        while (i < code.length - 1) {
          if (code[i] === "*" && code[i + 1] === "/") {
            i += 2;
            closed = true;
            break;
          }
          i++;
        }
        if (!closed) return { status: "incomplete", indent: "" };
        continue;
      }
      if (ch === "'" || ch === '"' || ch === "`") {
        const q = ch;
        i++;
        let closed = false;
        while (i < code.length) {
          if (code[i] === "\\" && q !== "`") {
            i += 2;
            continue;
          }
          if (code[i] === q) {
            i++;
            closed = true;
            break;
          }
          i++;
        }
        if (!closed) return { status: "incomplete", indent: "" };
        continue;
      }
      if (ch === "(" || ch === "[" || ch === "{") stack.push(ch);
      if (ch === ")") {
        if (stack.pop() !== "(") return { status: "invalid" };
      }
      if (ch === "]") {
        if (stack.pop() !== "[") return { status: "invalid" };
      }
      if (ch === "}") {
        if (stack.pop() !== "{") return { status: "invalid" };
      }
      i++;
    }
    if (stack.length === 0) return { status: "complete" };
    return { status: "incomplete", indent: "  " };
  }

  async function handleExecute(peerId, socket, msg) {
    const { header: parentHeader, content } = msg;
    currentParentHeader = parentHeader;
    currentAllowStdin = content.allow_stdin === true;

    const silent = content.silent || false;
    const storeHistory = content.store_history !== false;
    const code = content.code || "";

    if (!silent && storeHistory) executionCount++;

    await publishStatus("busy", parentHeader);

    // Publish execute_input
    const inputFrames = await encodeMsg(
      session,
      key,
      [],
      "execute_input",
      { code, execution_count: executionCount },
      parentHeader,
    );
    await iopub.send(inputFrames);

    // Ask REPL thread to evaluate
    let evalResult = null;
    try {
      evalResult = await op_jupyter_repl_evaluate(code);
    } catch (e) {
      // Evaluation threw (e.g. interrupted)
      const errFrames = await encodeMsg(
        session,
        key,
        [],
        "error",
        {
          ename: e?.name || "Error",
          evalue: e?.message || String(e),
          traceback: [],
        },
        parentHeader,
      );
      await iopub.send(errFrames);
      const replyContent = {
        status: "error",
        execution_count: executionCount,
        ename: e?.name || "Error",
        evalue: e?.message || String(e),
        traceback: [],
      };
      const replyFrames = await encodeMsg(
        session,
        key,
        [],
        "execute_reply",
        replyContent,
        parentHeader,
      );
      await socket.send(peerId, replyFrames);
      await publishStatus("idle", parentHeader);
      return;
    }

    if (evalResult !== null && evalResult !== undefined) {
      // Check for exception
      const exDetails = evalResult?.value?.exceptionDetails;
      if (exDetails) {
        // Exception during execution
        const exception = exDetails.exception;
        let ename = "Error";
        let evalue = "(none)";
        let traceback = [];

        if (exception) {
          const callResult = await op_jupyter_repl_call_function_on_args(
            `function(object) {
              if (object instanceof Error) {
                const name = "name" in object ? String(object.name) : "";
                const message = "message" in object ? String(object.message) : "";
                const stack = "stack" in object ? String(object.stack) : "";
                return JSON.stringify({ name, message, stack });
              } else {
                return JSON.stringify({ name: "", message: String(object), stack: "" });
              }
            }`,
            [exception],
          );
          if (callResult?.result?.value) {
            try {
              const parsed = JSON.parse(callResult.result.value);
              ename = parsed.name || "Error";
              evalue = parsed.message || "(none)";
              traceback = (parsed.stack || "").split("\n");
            } catch { /**/ }
          }
        } else {
          ename = exDetails.text || "Error";
          evalue = exDetails.text || "(none)";
        }

        const errFrames = await encodeMsg(
          session,
          key,
          [],
          "error",
          { ename, evalue, traceback },
          parentHeader,
        );
        await iopub.send(errFrames);

        const replyFrames = await encodeMsg(
          session,
          key,
          [],
          "execute_reply",
          {
            status: "error",
            execution_count: executionCount,
            ename,
            evalue,
            traceback,
          },
          parentHeader,
        );
        await socket.send(peerId, replyFrames);
      } else {
        // Success: publish the result
        const result = evalResult?.value?.result;
        if (result && !silent) {
          const arg0 = { value: executionCount };
          const arg1 = result.objectId
            ? { objectId: result.objectId }
            : { value: result.value };
          await op_jupyter_repl_call_function_on(arg0, arg1);
        }

        const replyFrames = await encodeMsg(
          session,
          key,
          [],
          "execute_reply",
          {
            status: "ok",
            execution_count: executionCount,
            payload: [],
            user_expressions: {},
          },
          parentHeader,
        );
        await socket.send(peerId, replyFrames);
      }
    } else {
      // Null result means eval was skipped or interrupted
      const replyFrames = await encodeMsg(
        session,
        key,
        [],
        "execute_reply",
        {
          status: "error",
          execution_count: executionCount,
          ename: "Error",
          evalue: "Execution failed",
          traceback: [],
        },
        parentHeader,
      );
      await socket.send(peerId, replyFrames);
    }

    await publishStatus("idle", parentHeader);
  }

  async function shellLoop(socket) {
    while (!shuttingDown) {
      const { peerId, frames } = await socket.recv();
      try {
        await handleShellMessage(socket, peerId, frames);
      } catch (err) {
        // A transient peer disconnect (e.g. a client that drops between
        // request and reply during first launch) must not kill the shell
        // loop, otherwise the kernel hangs and never answers another
        // request. The peer reconnects and resends. See denoland/deno#20542.
        void err;
      }
    }
  }

  async function handleShellMessage(socket, peerId, frames) {
    const msg = decodeMsg(frames);
    if (!(await hmacVerify(key, msg.signedParts, msg.sig))) {
      // Drop messages whose HMAC signature doesn't match the connection key.
      return;
    }
    const msgType = msg.header?.msg_type;
    const parentHeader = msg.header;

    // execute_request manages its own busy/idle status via handleExecute.
    // For other request types, publish busy here and idle in `finally`.
    if (msgType === "execute_request") {
      await handleExecute(peerId, socket, msg);
      return;
    }

    await publishStatus("busy", parentHeader);

    try {
      if (msgType === "kernel_info_request") {
        const replyFrames = await encodeMsg(
          session,
          key,
          [],
          "kernel_info_reply",
          kernelInfo(),
          parentHeader,
        );
        await socket.send(peerId, replyFrames);
      } else if (msgType === "complete_request") {
        const userCode = msg.content?.code || "";
        // `cursor_pos` is in Unicode codepoints; convert to a UTF-16 index for
        // JS string slicing (a 0 cursor means the start of the cell, so use
        // `??` rather than `||`).
        const cursorPosCp = msg.content?.cursor_pos ??
          utf16ToCodePointIndex(userCode, userCode.length);
        const cursorPos = codePointToUtf16Index(userCode, cursorPosCp);
        const expr = getExprFromLineAtPos(userCode, cursorPos);

        let completions = [];
        let cursorStart = cursorPos;

        if (expr.includes(".")) {
          const dotIdx = expr.lastIndexOf(".");
          const subExpr = expr.slice(0, dotIdx);
          const propName = expr.slice(dotIdx + 1);
          const props = await getExprProperties(subExpr);
          completions = props.filter((n) =>
            !n.startsWith("Symbol(") && n.startsWith(propName)
          );
          cursorStart = cursorPos - propName.length;
        } else {
          const globalProps = await getExprProperties("globalThis");
          const lexicalNames = await getLexicalScopeNames();
          const allNames = [...new Set([...globalProps, ...lexicalNames])];
          completions = allNames.filter((n) => n.startsWith(expr)).sort();
          cursorStart = cursorPos - expr.length;
        }

        const replyFrames = await encodeMsg(
          session,
          key,
          [],
          "complete_reply",
          {
            status: "ok",
            matches: completions,
            // Report cursor positions back in codepoints, as the frontend
            // expects.
            cursor_start: utf16ToCodePointIndex(userCode, cursorStart),
            cursor_end: cursorPosCp,
            metadata: {},
          },
          parentHeader,
        );
        await socket.send(peerId, replyFrames);
      } else if (msgType === "is_complete_request") {
        const result = checkIsComplete(msg.content?.code || "");
        const replyFrames = await encodeMsg(
          session,
          key,
          [],
          "is_complete_reply",
          result,
          parentHeader,
        );
        await socket.send(peerId, replyFrames);
      } else if (msgType === "inspect_request") {
        const replyFrames = await encodeMsg(
          session,
          key,
          [],
          "inspect_reply",
          {
            status: "ok",
            found: false,
            data: {},
            metadata: {},
          },
          parentHeader,
        );
        await socket.send(peerId, replyFrames);
      } else if (msgType === "history_request") {
        const replyFrames = await encodeMsg(
          session,
          key,
          [],
          "history_reply",
          { status: "ok", history: [] },
          parentHeader,
        );
        await socket.send(peerId, replyFrames);
      } else if (msgType === "comm_info_request") {
        const replyFrames = await encodeMsg(
          session,
          key,
          [],
          "comm_info_reply",
          { status: "ok", comms: {} },
          parentHeader,
        );
        await socket.send(peerId, replyFrames);
      } else if (msgType === "comm_open") {
        const replyFrames = await encodeMsg(
          session,
          key,
          [],
          "comm_close",
          { comm_id: msg.content?.comm_id, data: {} },
          parentHeader,
        );
        await socket.send(peerId, replyFrames);
      }
    } finally {
      await publishStatus("idle", parentHeader);
    }
  }

  async function controlLoop(socket) {
    while (true) {
      const { peerId, frames } = await socket.recv();
      try {
        await handleControlMessage(socket, peerId, frames);
      } catch (err) {
        // Transient read/send errors (e.g. a peer disconnecting during
        // first launch) must not kill the control loop, otherwise the
        // kernel hangs and never accepts another shutdown/interrupt
        // request. See denoland/deno#20542.
        void err;
      }
    }
  }

  async function handleControlMessage(socket, peerId, frames) {
    const msg = decodeMsg(frames);
    if (!(await hmacVerify(key, msg.signedParts, msg.sig))) {
      // Drop messages whose HMAC signature doesn't match the connection key.
      return;
    }
    const msgType = msg.header?.msg_type;
    const parentHeader = msg.header;

    if (msgType === "kernel_info_request") {
      const replyFrames = await encodeMsg(
        session,
        key,
        [],
        "kernel_info_reply",
        kernelInfo(),
        parentHeader,
      );
      await socket.send(peerId, replyFrames);
    } else if (msgType === "shutdown_request") {
      const restart = msg.content?.restart || false;
      const replyFrames = await encodeMsg(
        session,
        key,
        [],
        "shutdown_reply",
        { status: "ok", restart },
        parentHeader,
      );
      await socket.send(peerId, replyFrames);
      shuttingDown = true;
      // The Jupyter protocol expects the kernel process to exit after
      // sending a shutdown reply. Even on restart the frontend spawns a
      // fresh kernel, so the current process must exit either way;
      // otherwise it lingers as an orphan. Give the reply a moment to
      // flush over TCP, then exit. See denoland/deno#20556.
      setTimeout(() => Deno.exit(0), 100);
    } else if (msgType === "interrupt_request") {
      op_jupyter_repl_interrupt();
      const replyFrames = await encodeMsg(
        session,
        key,
        [],
        "interrupt_reply",
        { status: "ok" },
        parentHeader,
      );
      await socket.send(peerId, replyFrames);
    } else if (msgType === "debug_request") {
      // Not supported
    }
  }

  // Completion helpers
  async function getExprProperties(expr) {
    const evalResp = await op_jupyter_repl_get_properties(
      await evaluateExprForObjectId(expr),
    );
    if (!evalResp?.result) return [];
    return evalResp.result.map((p) => p.name);
  }

  async function evaluateExprForObjectId(expr) {
    // Evaluate the expression just to get objectId
    try {
      const resp = await op_jupyter_repl_evaluate(
        `(${expr})`, // wrap to handle expressions like "globalThis"
      );
      return resp?.value?.result?.objectId || null;
    } catch {
      return null;
    }
  }

  async function getLexicalScopeNames() {
    const resp = await op_jupyter_repl_global_lexical_scope_names();
    return resp?.names || [];
  }

  function getExprFromLineAtPos(line, cursorPos) {
    const sub = line.slice(0, cursorPos);
    const start = sub.search(/[\w$._]+$/);
    if (start === -1) return "";
    return sub.slice(start);
  }

  // Jupyter's `cursor_pos` is measured in Unicode codepoints, but JS strings
  // are indexed by UTF-16 code units. Convert a codepoint offset into the
  // equivalent UTF-16 index so slicing lands on a valid boundary even when the
  // code contains multi-byte / astral characters (see denoland/deno#22771).
  function codePointToUtf16Index(str, cpOffset) {
    let utf16 = 0;
    let cp = 0;
    while (cp < cpOffset && utf16 < str.length) {
      utf16 += str.codePointAt(utf16) > 0xffff ? 2 : 1;
      cp++;
    }
    return utf16;
  }

  // Inverse of codePointToUtf16Index: convert a UTF-16 index back to a codepoint
  // offset, used when reporting `cursor_start`/`cursor_end` to the frontend.
  function utf16ToCodePointIndex(str, utf16Offset) {
    let utf16 = 0;
    let cp = 0;
    while (utf16 < utf16Offset && utf16 < str.length) {
      utf16 += str.codePointAt(utf16) > 0xffff ? 2 : 1;
      cp++;
    }
    return cp;
  }

  // Services REPL-originated input_request messages: send them to the
  // frontend over the stdin ROUTER and forward the input_reply value back
  // through the response channel parked in op_state.
  async function stdinLoop() {
    while (!shuttingDown) {
      const req = await op_jupyter_recv_input();
      if (req === null || req === undefined) break;

      let value = null;
      try {
        value = await requestInput(req);
      } catch (err) {
        // A transient stdin transport error (e.g. a frontend that dropped
        // mid-prompt) must not abort the loop or, worse, leave the REPL
        // thread blocked forever waiting for a reply. See denoland/deno#20542.
        void err;
      }
      // Always answer the REPL thread exactly once so it can resume.
      op_jupyter_send_input_reply(value);
    }
  }

  async function requestInput(req) {
    if (!currentAllowStdin || !currentParentHeader.msg_id) {
      return null;
    }

    // Wait briefly for a frontend to connect to stdin if none has yet.
    if (stdin.peers.size === 0) {
      await new Promise((r) => setTimeout(r, 100));
    }
    if (stdin.peers.size === 0) {
      return null;
    }

    const reqFrames = await encodeMsg(
      session,
      key,
      [],
      "input_request",
      { prompt: req.prompt, password: req.password },
      currentParentHeader,
    );
    await stdin.sendAll(reqFrames);

    while (true) {
      const { frames } = await stdin.recv();
      try {
        const reply = decodeMsg(frames);
        if (!(await hmacVerify(key, reply.signedParts, reply.sig))) {
          // Ignore an input_reply whose HMAC signature doesn't verify and keep
          // waiting for a valid one, so a forged frame can't cancel a
          // legitimate input prompt. This matches the shell/control paths,
          // which drop bad messages and stay alive.
          continue;
        }
        if (reply.header?.msg_type === "input_reply") {
          const raw = reply.content?.value;
          return typeof raw === "string" ? raw : null;
        }
      } catch {
        return null;
      }
    }
  }

  // Start the loops concurrently
  await Promise.all([
    shellLoop(shell),
    controlLoop(control),
    stdinLoop(),
  ]);
}

internals.startJupyterKernel = startJupyterKernel;
