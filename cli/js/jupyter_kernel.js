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
  for (let i = 0; i < frames.length; i++) {
    const more = i < frames.length - 1;
    await writeAll(conn, makeShortFrame(frames[i], more));
  }
}

// --- ZMTP 3.1 handshake --------------------------------------------------------

function makeGreeting(socketType, asServer) {
  const buf = new Uint8Array(64);
  buf[0] = 0xff;
  // bytes 1..8 are padding zeros
  buf[8] = 0x01;
  buf[9] = 0x7f;
  buf[10] = 0x03; // version major
  buf[11] = 0x01; // version minor
  const mech = ENC.encode("NULL");
  buf.set(mech, 12);
  buf[32] = asServer ? 1 : 0;
  // rest is zeros (filler)
  return buf;
}

function makeReadyCommand(socketType) {
  const sockBytes = ENC.encode(socketType);
  const nameBytes = ENC.encode("READY");
  const propName = ENC.encode("Socket-Type");

  // property encoding: <len1:propNameLen><propName><len4:valueLen><value>
  const propLen = 1 + propName.length + 4 + sockBytes.length;
  const body = new Uint8Array(nameBytes.length + propLen);
  let o = 0;
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

async function zmtpHandshake(conn, socketType, asServer) {
  const greeting = makeGreeting(socketType, asServer);
  await writeAll(conn, greeting);

  // Read peer's greeting (64 bytes)
  await readExact(conn, 64);

  // Send READY command
  await writeAll(conn, makeReadyCommand(socketType));

  // Read peer's READY command (skip it)
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
  const header = JSON.parse(DEC.decode(frames[delimIdx + 2]));
  const parentHeader = JSON.parse(DEC.decode(frames[delimIdx + 3]));
  const metadata = JSON.parse(DEC.decode(frames[delimIdx + 4]));
  const content = JSON.parse(DEC.decode(frames[delimIdx + 5]));
  const buffers = frames.slice(delimIdx + 6);

  return { identities, sig, header, parentHeader, metadata, content, buffers };
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
async function runHeartbeat(port, ip) {
  const listener = Deno.listen({ hostname: ip, port });
  while (true) {
    const conn = await listener.accept();
    (async () => {
      try {
        await zmtpHandshake(conn, "REP", true);
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

async function runRouter(port, ip, incomingQueue, outgoingQueue) {
  const listener = Deno.listen({ hostname: ip, port });
  while (true) {
    const conn = await listener.accept();
    (async () => {
      try {
        await zmtpHandshake(conn, "ROUTER", true);

        // Generate peer identity
        const peerId = crypto.getRandomValues(new Uint8Array(5));

        // reader loop
        const readerLoop = (async () => {
          while (true) {
            const frames = await recvMultipart(conn);
            // Prepend peer identity (ROUTER semantics)
            incomingQueue.push([peerId, ...frames]);
          }
        })();

        // writer loop - deliver outgoing frames addressed to this peer
        const peerConnections = globalThis.__peerConnections__ ||
          (globalThis.__peerConnections__ = new Map());
        peerConnections.set(peerId, conn);

        await readerLoop;
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
 * Simple per-connection ROUTER: each accepted connection maps to a peer.
 * Incoming messages are pushed to the queue with the peer id prepended.
 * Outgoing messages from the queue are matched to the correct peer by first frame.
 */
class RouterSocket {
  constructor(port, ip) {
    this.port = port;
    this.ip = ip;
    this.incoming = makeQueue();
    this.peers = new Map(); // peerId (string) -> conn
    this._listen();
  }

  _listen() {
    (async () => {
      const listener = Deno.listen({ hostname: this.ip, port: this.port });
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
        await zmtpHandshake(conn, "ROUTER", true);
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
    await sendFrames(conn, frames);
  }
}

/**
 * PUB socket server.
 * Sends the same frames to all connected subscribers.
 */
class PubSocket {
  constructor(port, ip) {
    this.port = port;
    this.ip = ip;
    this.conns = new Set();
    this._listen();
  }

  _listen() {
    (async () => {
      const listener = Deno.listen({ hostname: this.ip, port: this.port });
      while (true) {
        const conn = await listener.accept();
        (async () => {
          try {
            await zmtpHandshake(conn, "PUB", true);
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
  const { ip, key, hb_port, shell_port, control_port, stdin_port, iopub_port } =
    info;
  const session = crypto.randomUUID();

  // Start heartbeat (purely async, fire-and-forget)
  runHeartbeat(hb_port, ip);

  const shell = new RouterSocket(shell_port, ip);
  const control = new RouterSocket(control_port, ip);
  const iopub = new PubSocket(iopub_port, ip);
  // Bind stdin port so frontends can connect. Input requests are not
  // supported in this kernel, so received messages are discarded.
  new RouterSocket(stdin_port, ip);

  let executionCount = 0;
  let currentParentHeader = {};
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

  async function sendReply(
    socket,
    peerId,
    msgType,
    content,
    parentHeader,
    metadata = {},
  ) {
    const frames = await encodeMsg(
      session,
      key,
      [peerId],
      msgType,
      content,
      parentHeader,
      metadata,
    );
    // The first frame is the identity; send without it as the socket adds identity
    // Actually send WITH the identity as the routing prefix for ROUTER
    await socket.send(peerId, frames.slice(1)); // skip identity since RouterSocket.send prepends it? No-frames[0] IS the identity
    // Actually let's just send all frames including the identity since ROUTER peer matching is by peerId arg
    // Re-think: we call socket.send(peerId, framesWithoutLeadingIdentity)
    // encodeMsg with identities=[peerId] produces [peerId, DELIMITER, ...]
    // RouterSocket.send(peerId, frames) should send the frames directly.
    // The peer's receiving side (DEALER) expects no leading identity.
    // So strip the identity from encodeMsg output:
  }

  // Actually let's simplify the send logic:
  async function routerSend(
    socket,
    peerId,
    msgType,
    content,
    parentHeader,
    metadata = {},
  ) {
    const frames = await encodeMsg(
      session,
      key,
      [], // no identities in the payload itself
      msgType,
      content,
      parentHeader,
      metadata,
    );
    // Prepend identity frame for ROUTER?DEALER routing
    // The DEALER peer expects: [DELIMITER, sig, header, ...]
    // ROUTER adds its routing frame as first; but since we're sending directly
    // on the raw TCP connection via sendFrames, we send all frames as-is.
    // The DEALER client interprets: first frame = routing (for sub-addressing), then DELIMITER...
    // Actually in ZMQ ROUTER?DEALER: ROUTER sends [identity, DELIMITER, sig, header, ...]
    // DEALER receives [DELIMITER, sig, header, ...] (strips identity automatically)
    // Since we're implementing TCP directly, we need to match this.
    const fullFrames = [peerId, ...frames];
    await socket.send(peerId, fullFrames);
  }

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
      await socket.send(peerId, [peerId, ...replyFrames]);
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
        await socket.send(peerId, [peerId, ...replyFrames]);
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
        await socket.send(peerId, [peerId, ...replyFrames]);
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
      await socket.send(peerId, [peerId, ...replyFrames]);
    }

    await publishStatus("idle", parentHeader);
  }

  async function handleShellMsg(peerId, socket) {
    const { frames } = await socket.recv();
    // Wait for more messages on that peer - but since we get {peerId, frames} from recv():
    // Actually recv() returns {peerId, peerKey, frames}
    const decodedPeerId = peerId; // already have it
    void decodedPeerId;
    // This function is called already with {peerId, frames}
    // We need to restructure...
  }

  async function shellLoop(socket) {
    while (!shuttingDown) {
      const { peerId, frames } = await socket.recv();
      const msg = decodeMsg(frames);
      const msgType = msg.header?.msg_type;
      const parentHeader = msg.header;

      // execute_request manages its own busy/idle status via handleExecute.
      // For other request types, publish busy here and idle in `finally`.
      if (msgType === "execute_request") {
        await handleExecute(peerId, socket, msg);
        continue;
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
          await socket.send(peerId, [peerId, ...replyFrames]);
        } else if (msgType === "complete_request") {
          const userCode = msg.content?.code || "";
          const cursorPos = msg.content?.cursor_pos || userCode.length;
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
              cursor_start: cursorStart,
              cursor_end: cursorPos,
              metadata: {},
            },
            parentHeader,
          );
          await socket.send(peerId, [peerId, ...replyFrames]);
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
          await socket.send(peerId, [peerId, ...replyFrames]);
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
          await socket.send(peerId, [peerId, ...replyFrames]);
        } else if (msgType === "history_request") {
          const replyFrames = await encodeMsg(
            session,
            key,
            [],
            "history_reply",
            { status: "ok", history: [] },
            parentHeader,
          );
          await socket.send(peerId, [peerId, ...replyFrames]);
        } else if (msgType === "comm_info_request") {
          const replyFrames = await encodeMsg(
            session,
            key,
            [],
            "comm_info_reply",
            { status: "ok", comms: {} },
            parentHeader,
          );
          await socket.send(peerId, [peerId, ...replyFrames]);
        } else if (msgType === "comm_open") {
          const replyFrames = await encodeMsg(
            session,
            key,
            [],
            "comm_close",
            { comm_id: msg.content?.comm_id, data: {} },
            parentHeader,
          );
          await socket.send(peerId, [peerId, ...replyFrames]);
        }
      } finally {
        await publishStatus("idle", parentHeader);
      }
    }
  }

  async function controlLoop(socket) {
    while (true) {
      const { peerId, frames } = await socket.recv();
      const msg = decodeMsg(frames);
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
        await socket.send(peerId, [peerId, ...replyFrames]);
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
        await socket.send(peerId, [peerId, ...replyFrames]);
        shuttingDown = true;
        if (!restart) {
          // Give a moment for the reply to be sent, then exit
          setTimeout(() => Deno.exit(0), 100);
        }
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
        await socket.send(peerId, [peerId, ...replyFrames]);
      } else if (msgType === "debug_request") {
        // Not supported
      }
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

  // Actually, let's use a simpler approach for getExprProperties:
  async function getExprPropertiesSimple(objectExpr) {
    // Evaluate to get object id, then get properties
    let result = null;
    try {
      // We need objectId for getProperties; use a small evaluate
      // The op_jupyter_repl_evaluate returns the full eval response
      const evalResp = await op_jupyter_repl_evaluate(`(${objectExpr})`);
      const objectId = evalResp?.value?.result?.objectId;
      if (!objectId) return [];
      const propsResp = await op_jupyter_repl_get_properties(objectId);
      if (!propsResp?.result) return [];
      return propsResp.result.map((p) => p.name);
    } catch {
      return [];
    }
  }

  function getExprFromLineAtPos(line, cursorPos) {
    const sub = line.slice(0, cursorPos);
    const start = sub.search(/[\w$._]+$/);
    if (start === -1) return "";
    return sub.slice(start);
  }

  // Start the loops concurrently
  await Promise.all([
    shellLoop(shell),
    controlLoop(control),
  ]);
}

internals.startJupyterKernel = startJupyterKernel;
