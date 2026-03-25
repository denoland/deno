// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import type { Span } from "ext:deno_telemetry/telemetry.ts";
import { builtinMeter, METRICS_ENABLED } from "ext:deno_telemetry/telemetry.ts";

const {
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  DateNow,
  JSONParse,
  JSONStringify,
  MapPrototypeGet,
  MapPrototypeSet,
  Number,
  SafeMap,
  SafeRegExp,
  String,
  StringPrototypeEndsWith,
  StringPrototypeIncludes,
  StringPrototypeIndexOf,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  StringPrototypeSubstring,
  StringPrototypeTrim,
  RegExpPrototypeTest,
} = primordials;

// --- Provider detection ---

const PROVIDER_MAP = new SafeMap<string, string>([
  ["api.openai.com", "openai"],
  ["api.anthropic.com", "anthropic"],
  ["generativelanguage.googleapis.com", "gcp.gemini"],
  ["api.cohere.com", "cohere"],
  ["api.mistral.ai", "mistral_ai"],
  ["api.groq.com", "groq"],
  ["api.deepseek.com", "deepseek"],
  ["api.x.ai", "x_ai"],
  ["api.perplexity.ai", "perplexity"],
  ["openrouter.ai", "openrouter"],
]);

const BEDROCK_RE = new SafeRegExp(
  "^bedrock-runtime\\.[a-z0-9-]+\\.amazonaws\\.com$",
);

interface GenAIInfo {
  providerName: string;
  operation: string;
  serverAddress: string;
  serverPort: number;
}

let customProviders: Map<string, string> | null = null;
let customProvidersLoaded = false;

function loadCustomProviders(): void {
  if (customProvidersLoaded) return;
  customProvidersLoaded = true;
  try {
    const envVal = Deno.env.get("OTEL_DENO_GENAI_CUSTOM_PROVIDERS");
    if (!envVal) return;
    customProviders = new SafeMap();
    const pairs = StringPrototypeSplit(envVal, ",");
    for (let i = 0; i < pairs.length; i++) {
      const eqIdx = StringPrototypeIndexOf(pairs[i], "=");
      if (eqIdx > 0) {
        MapPrototypeSet(
          customProviders,
          StringPrototypeTrim(StringPrototypeSubstring(pairs[i], 0, eqIdx)),
          StringPrototypeTrim(
            StringPrototypeSubstring(pairs[i], eqIdx + 1),
          ),
        );
      }
    }
  } catch {
    // env access may fail in some contexts
  }
}

function detectProvider(hostname: string): string | null {
  loadCustomProviders();
  if (customProviders) {
    const custom = MapPrototypeGet(customProviders, hostname);
    if (custom !== undefined) return custom;
  }
  const exact = MapPrototypeGet(PROVIDER_MAP, hostname);
  if (exact !== undefined) return exact;
  if (StringPrototypeEndsWith(hostname, ".openai.azure.com")) {
    return "azure.ai.openai";
  }
  if (RegExpPrototypeTest(BEDROCK_RE, hostname)) {
    return "aws.bedrock";
  }
  return null;
}

function detectOperation(
  method: string,
  pathname: string,
): string | null {
  if (method !== "POST") return null;
  if (
    StringPrototypeIncludes(pathname, "/chat/completions") ||
    StringPrototypeIncludes(pathname, "/messages")
  ) {
    return "chat";
  }
  if (StringPrototypeIncludes(pathname, "/embeddings")) {
    return "embeddings";
  }
  if (StringPrototypeIncludes(pathname, "/completions")) {
    return "text_completion";
  }
  return null;
}

export function detectGenAI(method: string, url: URL): GenAIInfo | null {
  // When running in deterministic test mode, allow localhost
  const providerName = detectProvider(url.hostname);
  if (providerName === null) return null;
  const operation = detectOperation(method, url.pathname);
  if (operation === null) return null;
  const defaultPort = url.protocol === "https:" ? 443 : 80;
  const port = url.port ? Number(url.port) : defaultPort;
  return {
    providerName,
    operation,
    serverAddress: url.hostname,
    serverPort: port,
  };
}

// --- Request body parsing ---

interface GenAIRequestData {
  model: string | undefined;
  temperature: number | undefined;
  topP: number | undefined;
  topK: number | undefined;
  maxTokens: number | undefined;
  frequencyPenalty: number | undefined;
  presencePenalty: number | undefined;
  isStreaming: boolean;
  // Content capture fields
  messages: unknown | undefined;
  systemInstructions: unknown | undefined;
  toolDefinitions: unknown | undefined;
}

let GENAI_CONTENT_CAPTURE: boolean | undefined;

function getContentCaptureEnabled(): boolean {
  if (GENAI_CONTENT_CAPTURE === undefined) {
    try {
      GENAI_CONTENT_CAPTURE =
        Deno.env.get("OTEL_GENAI_CAPTURE_MESSAGE_CONTENT") === "true";
    } catch {
      GENAI_CONTENT_CAPTURE = false;
    }
  }
  return GENAI_CONTENT_CAPTURE;
}

export function parseGenAIRequestBody(
  source: string | Uint8Array | null | undefined,
): GenAIRequestData | null {
  if (source == null) return null;
  try {
    const text = typeof source === "string" ? source : core.decode(source);
    const body = JSONParse(text);
    const captureContent = getContentCaptureEnabled();
    return {
      model: typeof body.model === "string" ? body.model : undefined,
      temperature: typeof body.temperature === "number"
        ? body.temperature
        : undefined,
      topP: typeof body.top_p === "number" ? body.top_p : undefined,
      topK: typeof body.top_k === "number" ? body.top_k : undefined,
      maxTokens: typeof body.max_tokens === "number"
        ? body.max_tokens
        : typeof body.max_completion_tokens === "number"
        ? body.max_completion_tokens
        : undefined,
      frequencyPenalty: typeof body.frequency_penalty === "number"
        ? body.frequency_penalty
        : undefined,
      presencePenalty: typeof body.presence_penalty === "number"
        ? body.presence_penalty
        : undefined,
      isStreaming: body.stream === true,
      messages: captureContent ? body.messages : undefined,
      systemInstructions: captureContent ? body.system : undefined,
      toolDefinitions: captureContent ? body.tools : undefined,
    };
  } catch {
    return null;
  }
}

// --- Span attribute helpers ---

export function setGenAIRequestAttributes(
  span: Span,
  info: GenAIInfo,
  data: GenAIRequestData | null,
): void {
  span.setAttribute("gen_ai.operation.name", info.operation);
  span.setAttribute("gen_ai.provider.name", info.providerName);
  span.setAttribute("server.address", info.serverAddress);
  span.setAttribute("server.port", String(info.serverPort));
  if (data === null) return;
  if (data.model !== undefined) {
    span.setAttribute("gen_ai.request.model", data.model);
  }
  if (data.temperature !== undefined) {
    span.setAttribute("gen_ai.request.temperature", String(data.temperature));
  }
  if (data.topP !== undefined) {
    span.setAttribute("gen_ai.request.top_p", String(data.topP));
  }
  if (data.topK !== undefined) {
    span.setAttribute("gen_ai.request.top_k", String(data.topK));
  }
  if (data.maxTokens !== undefined) {
    span.setAttribute("gen_ai.request.max_tokens", String(data.maxTokens));
  }
  if (data.frequencyPenalty !== undefined) {
    span.setAttribute(
      "gen_ai.request.frequency_penalty",
      String(data.frequencyPenalty),
    );
  }
  if (data.presencePenalty !== undefined) {
    span.setAttribute(
      "gen_ai.request.presence_penalty",
      String(data.presencePenalty),
    );
  }
}

// --- Response parsing ---

interface GenAIResponseData {
  id: string | undefined;
  model: string | undefined;
  finishReasons: string[];
  inputTokens: number | undefined;
  outputTokens: number | undefined;
  outputMessages: unknown | undefined;
}

function parseResponseBody(text: string): GenAIResponseData | null {
  try {
    const body = JSONParse(text);
    const captureContent = getContentCaptureEnabled();
    const finishReasons: string[] = [];
    let outputMessages: unknown | undefined;

    // OpenAI-style: choices[].finish_reason
    if (body.choices) {
      for (let i = 0; i < body.choices.length; i++) {
        const choice = body.choices[i];
        if (choice.finish_reason) {
          finishReasons[finishReasons.length] = choice.finish_reason;
        }
      }
      if (captureContent) {
        outputMessages = ArrayPrototypeMap(
          body.choices,
          (c: { message: unknown }) => c.message,
        );
      }
    }

    // Anthropic-style: stop_reason at top level
    if (body.stop_reason) {
      finishReasons[finishReasons.length] = body.stop_reason;
      if (captureContent) {
        outputMessages = body.content;
      }
    }

    // Token usage: OpenAI uses prompt_tokens/completion_tokens,
    // Anthropic uses input_tokens/output_tokens
    const usage = body.usage;
    let inputTokens: number | undefined;
    let outputTokens: number | undefined;
    if (usage) {
      inputTokens = usage.prompt_tokens ?? usage.input_tokens;
      outputTokens = usage.completion_tokens ?? usage.output_tokens;
    }

    return {
      id: typeof body.id === "string" ? body.id : undefined,
      model: typeof body.model === "string" ? body.model : undefined,
      finishReasons,
      inputTokens,
      outputTokens,
      outputMessages,
    };
  } catch {
    return null;
  }
}

function setGenAIResponseAttributes(
  span: Span,
  data: GenAIResponseData,
): void {
  if (data.id !== undefined) {
    span.setAttribute("gen_ai.response.id", data.id);
  }
  if (data.model !== undefined) {
    span.setAttribute("gen_ai.response.model", data.model);
  }
  if (data.finishReasons.length > 0) {
    span.setAttribute(
      "gen_ai.response.finish_reasons",
      data.finishReasons,
    );
  }
  if (data.inputTokens !== undefined) {
    span.setAttribute("gen_ai.usage.input_tokens", String(data.inputTokens));
  }
  if (data.outputTokens !== undefined) {
    span.setAttribute("gen_ai.usage.output_tokens", String(data.outputTokens));
  }
}

// --- Metrics ---

type Histogram = {
  record(value: number, attributes?: Record<string, string>): void;
};

let tokenUsageHistogram: Histogram | null = null;
let operationDurationHistogram: Histogram | null = null;

function getTokenUsageHistogram(): Histogram {
  if (tokenUsageHistogram === null) {
    tokenUsageHistogram = builtinMeter().createHistogram(
      "gen_ai.client.token.usage",
      {
        description: "Number of GenAI input and output tokens used",
        unit: "{token}",
      },
    );
  }
  return tokenUsageHistogram;
}

function getOperationDurationHistogram(): Histogram {
  if (operationDurationHistogram === null) {
    operationDurationHistogram = builtinMeter().createHistogram(
      "gen_ai.client.operation.duration",
      { description: "GenAI operation duration", unit: "s" },
    );
  }
  return operationDurationHistogram;
}

function recordMetrics(
  info: GenAIInfo,
  requestData: GenAIRequestData | null,
  responseData: GenAIResponseData | null,
  durationMs: number,
): void {
  if (!METRICS_ENABLED) return;

  const baseAttrs: Record<string, string> = {
    "gen_ai.operation.name": info.operation,
    "gen_ai.provider.name": info.providerName,
  };
  if (requestData?.model) {
    baseAttrs["gen_ai.request.model"] = requestData.model;
  }
  if (responseData?.model) {
    baseAttrs["gen_ai.response.model"] = responseData.model;
  }

  getOperationDurationHistogram().record(durationMs / 1000, baseAttrs);

  if (responseData?.inputTokens !== undefined) {
    getTokenUsageHistogram().record(responseData.inputTokens, {
      ...baseAttrs,
      "gen_ai.token.type": "input",
    });
  }
  if (responseData?.outputTokens !== undefined) {
    getTokenUsageHistogram().record(responseData.outputTokens, {
      ...baseAttrs,
      "gen_ai.token.type": "output",
    });
  }
}

// --- Events ---

function emitGenAIEvent(
  span: Span,
  info: GenAIInfo,
  requestData: GenAIRequestData | null,
  responseData: GenAIResponseData | null,
): void {
  if (!getContentCaptureEnabled()) return;

  const attrs: Record<string, string> = {
    "gen_ai.operation.name": info.operation,
  };

  if (requestData?.messages !== undefined) {
    attrs["gen_ai.input.messages"] = JSONStringify(requestData.messages);
  }
  if (requestData?.systemInstructions !== undefined) {
    attrs["gen_ai.system_instructions"] = JSONStringify(
      requestData.systemInstructions,
    );
  }
  if (requestData?.toolDefinitions !== undefined) {
    attrs["gen_ai.tool.definitions"] = JSONStringify(
      requestData.toolDefinitions,
    );
  }
  if (responseData?.outputMessages !== undefined) {
    attrs["gen_ai.output.messages"] = JSONStringify(
      responseData.outputMessages,
    );
  }
  if (responseData?.inputTokens !== undefined) {
    attrs["gen_ai.usage.input_tokens"] = String(responseData.inputTokens);
  }
  if (responseData?.outputTokens !== undefined) {
    attrs["gen_ai.usage.output_tokens"] = String(responseData.outputTokens);
  }

  span.addEvent("gen_ai.client.inference.operation.details", attrs);
}

// --- Non-streaming response instrumentation ---

export function instrumentGenAIResponse(
  span: Span,
  response: Response,
  info: GenAIInfo,
  requestData: GenAIRequestData | null,
  startTime: number,
): void {
  try {
    const clone = response.clone();
    // deno-lint-ignore prefer-primordials
    clone.text().then(
      (text) => {
        try {
          const responseData = parseResponseBody(text);
          if (responseData !== null) {
            setGenAIResponseAttributes(span, responseData);
          }
          recordMetrics(info, requestData, responseData, DateNow() - startTime);
          emitGenAIEvent(span, info, requestData, responseData);
        } catch {
          // best-effort
        }
        span.end();
      },
      () => {
        recordMetrics(info, requestData, null, DateNow() - startTime);
        span.end();
      },
    );
  } catch {
    recordMetrics(info, requestData, null, DateNow() - startTime);
    span.end();
  }
}

// --- SSE streaming response instrumentation ---

class SSEAccumulator {
  #buffer = "";
  #id: string | undefined;
  #model: string | undefined;
  #finishReasons: string[] = [];
  #inputTokens: number | undefined;
  #outputTokens: number | undefined;
  #outputChunks: string[] = [];
  #captureContent: boolean;

  constructor() {
    this.#captureContent = getContentCaptureEnabled();
  }

  processChunk(chunk: Uint8Array): void {
    this.#buffer += core.decode(chunk);
    // Process complete lines
    let newlineIdx;
    while (
      (newlineIdx = StringPrototypeIndexOf(this.#buffer, "\n")) !== -1
    ) {
      const line = StringPrototypeSubstring(this.#buffer, 0, newlineIdx);
      this.#buffer = StringPrototypeSubstring(this.#buffer, newlineIdx + 1);
      this.#processLine(line);
    }
  }

  #processLine(line: string): void {
    if (!StringPrototypeStartsWith(line, "data: ")) return;
    const data = StringPrototypeTrim(StringPrototypeSubstring(line, 6));
    if (data === "[DONE]") return;
    try {
      const parsed = JSONParse(data);

      if (parsed.id && !this.#id) this.#id = parsed.id;
      if (parsed.model && !this.#model) this.#model = parsed.model;

      // OpenAI-style streaming chunks
      if (parsed.choices) {
        for (let i = 0; i < parsed.choices.length; i++) {
          const choice = parsed.choices[i];
          if (choice.finish_reason) {
            this.#finishReasons[this.#finishReasons.length] =
              choice.finish_reason;
          }
          if (this.#captureContent && choice.delta?.content) {
            this.#outputChunks[this.#outputChunks.length] =
              choice.delta.content;
          }
        }
      }

      // Anthropic-style streaming
      if (parsed.type === "message_delta" && parsed.delta?.stop_reason) {
        this.#finishReasons[this.#finishReasons.length] =
          parsed.delta.stop_reason;
      }
      if (
        this.#captureContent && parsed.type === "content_block_delta" &&
        parsed.delta?.text
      ) {
        this.#outputChunks[this.#outputChunks.length] = parsed.delta.text;
      }

      // Usage data (often in the last chunk)
      if (parsed.usage) {
        const u = parsed.usage;
        if (u.prompt_tokens !== undefined) this.#inputTokens = u.prompt_tokens;
        if (u.input_tokens !== undefined) this.#inputTokens = u.input_tokens;
        if (u.completion_tokens !== undefined) {
          this.#outputTokens = u.completion_tokens;
        }
        if (u.output_tokens !== undefined) {
          this.#outputTokens = u.output_tokens;
        }
      }

      // Anthropic message_start has usage in the message
      if (parsed.type === "message_start" && parsed.message?.usage) {
        const u = parsed.message.usage;
        if (u.input_tokens !== undefined) this.#inputTokens = u.input_tokens;
      }
      if (parsed.type === "message_delta" && parsed.usage) {
        const u = parsed.usage;
        if (u.output_tokens !== undefined) {
          this.#outputTokens = u.output_tokens;
        }
      }
    } catch {
      // Not valid JSON, skip
    }
  }

  getResult(): GenAIResponseData {
    let outputMessages: unknown | undefined;
    if (this.#captureContent && this.#outputChunks.length > 0) {
      outputMessages = [{
        content: ArrayPrototypeJoin(this.#outputChunks, ""),
      }];
    }
    return {
      id: this.#id,
      model: this.#model,
      finishReasons: this.#finishReasons,
      inputTokens: this.#inputTokens,
      outputTokens: this.#outputTokens,
      outputMessages,
    };
  }
}

export function wrapStreamingResponse(
  span: Span,
  stream: ReadableStream<Uint8Array>,
  info: GenAIInfo,
  requestData: GenAIRequestData | null,
  startTime: number,
): ReadableStream<Uint8Array> {
  const accumulator = new SSEAccumulator();
  const transformStream = new TransformStream<Uint8Array, Uint8Array>({
    transform(chunk, controller) {
      accumulator.processChunk(chunk);
      controller.enqueue(chunk);
    },
    flush() {
      const responseData = accumulator.getResult();
      setGenAIResponseAttributes(span, responseData);
      recordMetrics(info, requestData, responseData, DateNow() - startTime);
      emitGenAIEvent(span, info, requestData, responseData);
      span.end();
    },
  });
  return stream.pipeThrough(transformStream);
}
