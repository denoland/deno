// Copyright 2018-2026 the Deno authors. MIT license.

// Implementation of the unstable `Deno.McpServer` API: a minimal Model
// Context Protocol server (JSON-RPC 2.0 lifecycle, tools, resources and
// prompts) with two transports:
//
// - stdio: newline-delimited JSON-RPC messages over stdin/stdout
// - streamable HTTP: a `fetch`-style handler usable with `Deno.serve`

(function () {
const { core, primordials } = __bootstrap;
const { op_base64_encode } = core.ops;
const {
  ArrayIsArray,
  ArrayPrototypePush,
  JSONParse,
  JSONStringify,
  Error,
  SafeMap,
  SafeMapIterator,
  Symbol,
  TypeError,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeIndexOf,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeSubarray,
  Uint8Array,
  Uint8ArrayPrototype,
  ObjectPrototypeIsPrototypeOf,
} = primordials;

const { TextDecoder, TextEncoder } = core.loadExtScript(
  "ext:deno_web/08_text_encoding.js",
);
const io = core.loadExtScript("ext:deno_io/12_io.js");

let _responseImpl;
function lazyResponse() {
  return _responseImpl ??
    (_responseImpl = core.loadExtScript("ext:deno_fetch/23_response.js"));
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

// Most recent protocol revision this implementation targets, plus older
// revisions that are wire-compatible with the subset implemented here.
const LATEST_PROTOCOL_VERSION = "2025-06-18";
const SUPPORTED_PROTOCOL_VERSIONS = [
  "2025-11-25",
  "2025-06-18",
  "2025-03-26",
  "2024-11-05",
];

// JSON-RPC 2.0 error codes
const PARSE_ERROR = -32700;
const INVALID_REQUEST = -32600;
const METHOD_NOT_FOUND = -32601;
const INVALID_PARAMS = -32602;
const INTERNAL_ERROR = -32603;
// MCP-specific: resource not found
const RESOURCE_NOT_FOUND = -32002;

const NEWLINE = 10;

function concatBytes(a: Uint8Array, b: Uint8Array): Uint8Array {
  const aLen = TypedArrayPrototypeGetLength(a);
  const bLen = TypedArrayPrototypeGetLength(b);
  const out = new Uint8Array(aLen + bLen);
  TypedArrayPrototypeSet(out, a, 0);
  TypedArrayPrototypeSet(out, b, aLen);
  return out;
}

function rpcResult(id: unknown, result: unknown) {
  return { jsonrpc: "2.0", id, result };
}

function rpcError(id: unknown, code: number, message: string) {
  return { jsonrpc: "2.0", id, error: { code, message } };
}

function isPlainText(value: unknown): value is string {
  return typeof value === "string";
}

// Normalizes a tool handler return value into a CallToolResult.
function normalizeToolResult(value: unknown) {
  if (value === undefined || value === null) {
    return { content: [] };
  }
  if (isPlainText(value)) {
    return { content: [{ type: "text", text: value }] };
  }
  if (
    typeof value === "object" &&
    ArrayIsArray((value as { content: unknown }).content)
  ) {
    // Already a CallToolResult shaped object.
    return value;
  }
  // Structured value: serialize it for the text fallback and pass it
  // through as structuredContent.
  const text = JSONStringify(value);
  if (typeof value === "object" && !ArrayIsArray(value)) {
    return { content: [{ type: "text", text }], structuredContent: value };
  }
  return { content: [{ type: "text", text }] };
}

// Normalizes a resource handler return value into resource contents.
function normalizeResourceContents(
  uri: string,
  mimeType: string | undefined,
  value: unknown,
) {
  if (typeof value === "object" && value !== null) {
    const contents = (value as { contents: unknown }).contents;
    if (ArrayIsArray(contents)) {
      // Already a ReadResourceResult shaped object.
      return value;
    }
    if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, value)) {
      return {
        contents: [{
          uri,
          mimeType: mimeType ?? "application/octet-stream",
          blob: op_base64_encode(value),
        }],
      };
    }
  }
  return {
    contents: [{
      uri,
      mimeType: mimeType ?? "text/plain",
      text: isPlainText(value) ? value : JSONStringify(value),
    }],
  };
}

// Normalizes a prompt handler return value into a GetPromptResult.
function normalizePromptResult(
  description: string | undefined,
  value: unknown,
) {
  let messages;
  if (isPlainText(value)) {
    messages = [{ role: "user", content: { type: "text", text: value } }];
  } else if (ArrayIsArray(value)) {
    messages = [];
    for (let i = 0; i < value.length; i++) {
      const entry = value[i];
      if (isPlainText(entry)) {
        ArrayPrototypePush(messages, {
          role: "user",
          content: { type: "text", text: entry },
        });
      } else {
        ArrayPrototypePush(messages, entry);
      }
    }
  } else if (
    typeof value === "object" && value !== null &&
    ArrayIsArray((value as { messages: unknown }).messages)
  ) {
    // Already a GetPromptResult shaped object.
    return value;
  } else {
    throw new TypeError(
      "Prompt handler must return a string, an array of messages, or a result object",
    );
  }
  const result: { messages: unknown[]; description?: string } = { messages };
  if (description !== undefined) {
    result.description = description;
  }
  return result;
}

class McpServer {
  #name: string;
  #version: string;
  #instructions: string | undefined;
  #tools = new SafeMap();
  #resources = new SafeMap();
  #prompts = new SafeMap();

  fetch;

  constructor(options: {
    name: string;
    version: string;
    instructions?: string;
  }) {
    if (typeof options !== "object" || options === null) {
      throw new TypeError("McpServer requires an options object");
    }
    if (typeof options.name !== "string") {
      throw new TypeError("McpServer requires a string 'name' option");
    }
    if (typeof options.version !== "string") {
      throw new TypeError("McpServer requires a string 'version' option");
    }
    this.#name = options.name;
    this.#version = options.version;
    this.#instructions = options.instructions;
    this.fetch = (request: Request) => this.#handleHttpRequest(request);
  }

  tool(name: string, definition, handler?) {
    if (typeof definition === "function") {
      handler = definition;
      definition = { __proto__: null };
    }
    if (typeof name !== "string") {
      throw new TypeError("Tool name must be a string");
    }
    if (typeof handler !== "function") {
      throw new TypeError(`Tool "${name}" requires a handler function`);
    }
    this.#tools.set(name, { definition, handler });
    return this;
  }

  resource(uri: string, metadata, handler?) {
    if (typeof metadata === "function") {
      handler = metadata;
      metadata = { __proto__: null };
    }
    if (typeof uri !== "string") {
      throw new TypeError("Resource URI must be a string");
    }
    if (typeof handler !== "function") {
      throw new TypeError(`Resource "${uri}" requires a handler function`);
    }
    this.#resources.set(uri, { metadata, handler });
    return this;
  }

  prompt(name: string, definition, handler?) {
    if (typeof definition === "function") {
      handler = definition;
      definition = { __proto__: null };
    }
    if (typeof name !== "string") {
      throw new TypeError("Prompt name must be a string");
    }
    if (typeof handler !== "function") {
      throw new TypeError(`Prompt "${name}" requires a handler function`);
    }
    this.#prompts.set(name, { definition, handler });
    return this;
  }

  // Handles a single decoded JSON-RPC message. Returns a response object,
  // or null if the message was a notification (or a client response).
  async #handleMessage(message): Promise<object | null> {
    if (
      typeof message !== "object" || message === null ||
      message.jsonrpc !== "2.0"
    ) {
      return rpcError(null, INVALID_REQUEST, "Invalid request");
    }
    if (typeof message.method !== "string") {
      // A response to a server-initiated request; this server never sends
      // requests, so there is nothing to correlate it with.
      return null;
    }
    const isNotification = message.id === undefined;
    if (isNotification) {
      // notifications/initialized, notifications/cancelled, etc. None of
      // them require action in this implementation.
      return null;
    }
    const { id, method, params } = message;
    try {
      const result = await this.#dispatch(method, params ?? {});
      if (result === METHOD_NOT_FOUND_SENTINEL) {
        return rpcError(id, METHOD_NOT_FOUND, `Method not found: ${method}`);
      }
      return rpcResult(id, result);
    } catch (error) {
      if (ObjectPrototypeIsPrototypeOf(McpErrorPrototype, error)) {
        return rpcError(id, error.code, error.message);
      }
      return rpcError(
        id,
        INTERNAL_ERROR,
        `Internal error: ${error?.message ?? error}`,
      );
    }
  }

  async #dispatch(method: string, params): Promise<unknown> {
    switch (method) {
      case "initialize":
        return this.#initialize(params);
      case "ping":
        return {};
      case "tools/list":
        return this.#listTools();
      case "tools/call":
        return await this.#callTool(params);
      case "resources/list":
        return this.#listResources();
      case "resources/templates/list":
        return { resourceTemplates: [] };
      case "resources/read":
        return await this.#readResource(params);
      case "prompts/list":
        return this.#listPrompts();
      case "prompts/get":
        return await this.#getPrompt(params);
      default:
        return METHOD_NOT_FOUND_SENTINEL;
    }
  }

  #initialize(params) {
    let protocolVersion = LATEST_PROTOCOL_VERSION;
    const requested = params?.protocolVersion;
    for (let i = 0; i < SUPPORTED_PROTOCOL_VERSIONS.length; i++) {
      if (SUPPORTED_PROTOCOL_VERSIONS[i] === requested) {
        protocolVersion = requested;
        break;
      }
    }
    const result = {
      protocolVersion,
      capabilities: {
        tools: { listChanged: false },
        resources: { subscribe: false, listChanged: false },
        prompts: { listChanged: false },
      },
      serverInfo: { name: this.#name, version: this.#version },
    };
    if (this.#instructions !== undefined) {
      result.instructions = this.#instructions;
    }
    return result;
  }

  #listTools() {
    const tools = [];
    for (const { 0: name, 1: entry } of new SafeMapIterator(this.#tools)) {
      const definition = entry.definition;
      const tool = {
        name,
        inputSchema: definition.inputSchema ?? { type: "object" },
      };
      if (definition.title !== undefined) tool.title = definition.title;
      // deno-lint-ignore prefer-primordials
      if (definition?.description !== undefined) {
        // deno-lint-ignore prefer-primordials
        tool.description = definition?.description;
      }
      if (definition.outputSchema !== undefined) {
        tool.outputSchema = definition.outputSchema;
      }
      if (definition.annotations !== undefined) {
        tool.annotations = definition.annotations;
      }
      ArrayPrototypePush(tools, tool);
    }
    return { tools };
  }

  async #callTool(params) {
    const name = params?.name;
    const entry = this.#tools.get(name);
    if (entry === undefined) {
      throw new McpError(INVALID_PARAMS, `Unknown tool: ${name}`);
    }
    try {
      const value = await entry.handler(params?.arguments ?? {});
      return normalizeToolResult(value);
    } catch (error) {
      // Tool execution errors are reported in-band so the model can see
      // them, per the MCP specification.
      return {
        content: [{ type: "text", text: `${error?.message ?? error}` }],
        isError: true,
      };
    }
  }

  #listResources() {
    const resources = [];
    for (
      const { 0: uri, 1: entry } of new SafeMapIterator(this.#resources)
    ) {
      const metadata = entry.metadata;
      const resource = { uri, name: metadata.name ?? uri };
      if (metadata.title !== undefined) resource.title = metadata.title;
      // deno-lint-ignore prefer-primordials
      if (metadata?.description !== undefined) {
        // deno-lint-ignore prefer-primordials
        resource.description = metadata?.description;
      }
      if (metadata.mimeType !== undefined) {
        resource.mimeType = metadata.mimeType;
      }
      ArrayPrototypePush(resources, resource);
    }
    return { resources };
  }

  async #readResource(params) {
    const uri = params?.uri;
    const entry = this.#resources.get(uri);
    if (entry === undefined) {
      throw new McpError(RESOURCE_NOT_FOUND, `Resource not found: ${uri}`);
    }
    const value = await entry.handler(uri);
    return normalizeResourceContents(uri, entry.metadata.mimeType, value);
  }

  #listPrompts() {
    const prompts = [];
    for (const { 0: name, 1: entry } of new SafeMapIterator(this.#prompts)) {
      const definition = entry.definition;
      const prompt = { name };
      if (definition.title !== undefined) prompt.title = definition.title;
      // deno-lint-ignore prefer-primordials
      if (definition?.description !== undefined) {
        // deno-lint-ignore prefer-primordials
        prompt.description = definition?.description;
      }
      if (definition.arguments !== undefined) {
        prompt.arguments = definition.arguments;
      }
      ArrayPrototypePush(prompts, prompt);
    }
    return { prompts };
  }

  async #getPrompt(params) {
    const name = params?.name;
    const entry = this.#prompts.get(name);
    if (entry === undefined) {
      throw new McpError(INVALID_PARAMS, `Unknown prompt: ${name}`);
    }
    const value = await entry.handler(params?.arguments ?? {});
    // deno-lint-ignore prefer-primordials
    return normalizePromptResult(entry.definition?.description, value);
  }

  // stdio transport: newline-delimited JSON-RPC messages on stdin/stdout.
  async serve(options = { __proto__: null }) {
    const transport = options?.transport ?? "stdio";
    if (transport !== "stdio") {
      throw new TypeError(
        `Unsupported transport: "${transport}". Use "stdio", or pass server.fetch to Deno.serve() for HTTP`,
      );
    }
    let buffer = new Uint8Array(0);
    const chunk = new Uint8Array(16 * 1024);
    while (true) {
      const n = await io.stdin.read(chunk);
      if (n === null) {
        break;
      }
      buffer = concatBytes(buffer, TypedArrayPrototypeSubarray(chunk, 0, n));
      while (true) {
        const newlineIndex = TypedArrayPrototypeIndexOf(buffer, NEWLINE);
        if (newlineIndex === -1) {
          break;
        }
        const line = TypedArrayPrototypeSubarray(buffer, 0, newlineIndex);
        buffer = TypedArrayPrototypeSubarray(buffer, newlineIndex + 1);
        if (TypedArrayPrototypeGetLength(line) === 0) {
          continue;
        }
        await this.#handleStdioLine(line);
      }
    }
  }

  async #handleStdioLine(line: Uint8Array) {
    let message;
    try {
      message = JSONParse(decoder.decode(line));
    } catch {
      await this.#writeStdioMessage(
        rpcError(null, PARSE_ERROR, "Parse error"),
      );
      return;
    }
    if (ArrayIsArray(message)) {
      for (let i = 0; i < message.length; i++) {
        const response = await this.#handleMessage(message[i]);
        if (response !== null) {
          await this.#writeStdioMessage(response);
        }
      }
      return;
    }
    const response = await this.#handleMessage(message);
    if (response !== null) {
      await this.#writeStdioMessage(response);
    }
  }

  async #writeStdioMessage(message: object) {
    let bytes = encoder.encode(JSONStringify(message) + "\n");
    while (TypedArrayPrototypeGetLength(bytes) > 0) {
      const written = await io.stdout.write(bytes);
      bytes = TypedArrayPrototypeSubarray(bytes, written);
    }
  }

  // Streamable HTTP transport (stateless): JSON-RPC messages are POSTed to
  // the endpoint and answered with `application/json` responses. Pass
  // `server.fetch` to `Deno.serve()`.
  async #handleHttpRequest(request: Request): Promise<Response> {
    const { Response } = lazyResponse();
    if (request.method !== "POST") {
      return new Response(null, {
        status: 405,
        headers: { "allow": "POST" },
      });
    }
    let body;
    try {
      body = JSONParse(await request.text());
    } catch {
      return this.#jsonResponse(
        rpcError(null, PARSE_ERROR, "Parse error"),
        400,
      );
    }
    if (ArrayIsArray(body)) {
      const responses = [];
      for (let i = 0; i < body.length; i++) {
        const response = await this.#handleMessage(body[i]);
        if (response !== null) {
          ArrayPrototypePush(responses, response);
        }
      }
      if (responses.length === 0) {
        return new Response(null, { status: 202 });
      }
      return this.#jsonResponse(responses, 200);
    }
    const response = await this.#handleMessage(body);
    if (response === null) {
      return new Response(null, { status: 202 });
    }
    return this.#jsonResponse(response, 200);
  }

  #jsonResponse(value: unknown, status: number): Response {
    const { Response } = lazyResponse();
    return new Response(JSONStringify(value), {
      status,
      headers: { "content-type": "application/json" },
    });
  }
}

const METHOD_NOT_FOUND_SENTINEL = Symbol("methodNotFound");

class McpError extends Error {
  code: number;

  constructor(code: number, message: string) {
    super(message);
    this.code = code;
  }
}
const McpErrorPrototype = McpError.prototype;

return { McpServer };
})();
