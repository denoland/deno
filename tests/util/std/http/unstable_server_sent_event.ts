// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 *
 * Provides {@linkcode ServerSentEvent} and
 * {@linkcode ServerSentEventStreamTarget} which provides an interface to send
 * server sent events to a browser using the DOM event model.
 *
 * The {@linkcode ServerSentEventStreamTarget} provides the `.asResponse()` or
 * `.asResponseInit()` to provide a body and headers to the client to establish
 * the event connection. This is accomplished by keeping a connection open to
 * the client by not closing the body, which allows events to be sent down the
 * connection and processed by the client browser.
 *
 * See more about Server-sent events on [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events)
 *
 * ## Example
 *
 * ```ts
 * import {
 *   ServerSentEvent,
 *   ServerSentEventStreamTarget,
 * } from "https://deno.land/std@$STD_VERSION/http/unstable_server_sent_event.ts";
 *
 * Deno.serve({ port: 8000 }, (request) => {
 *   const target = new ServerSentEventStreamTarget();
 *   let counter = 0;
 *
 *   // Sends an event every 2 seconds, incrementing the ID
 *   const id = setInterval(() => {
 *     const evt = new ServerSentEvent(
 *       "message",
 *       { data: { hello: "world" }, id: counter++ },
 *     );
 *     target.dispatchEvent(evt);
 *   }, 2000);
 *
 *   target.addEventListener("close", () => clearInterval(id));
 *   return target.asResponse();
 * });
 * ```
 *
 * @module
 */

import { assert } from "../assert/assert.ts";

const encoder = new TextEncoder();

const DEFAULT_KEEP_ALIVE_INTERVAL = 30_000;

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 */
export interface ServerSentEventInit extends EventInit {
  /** Optional arbitrary data to send to the client, data this is a string will
   * be sent unmodified, otherwise `JSON.parse()` will be used to serialize the
   * value. */
  data?: unknown;

  /** An optional `id` which will be sent with the event and exposed in the
   * client `EventSource`. */
  id?: number;

  /** The replacer is passed to `JSON.stringify` when converting the `data`
   * property to a JSON string. */
  replacer?:
    | (string | number)[]
    // deno-lint-ignore no-explicit-any
    | ((this: any, key: string, value: any) => any);

  /** Space is passed to `JSON.stringify` when converting the `data` property
   * to a JSON string. */
  space?: string | number;
}

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 */
export interface ServerSentEventTargetOptions {
  /** Keep client connections alive by sending a comment event to the client
   * at a specified interval.  If `true`, then it polls every 30000 milliseconds
   * (30 seconds). If set to a number, then it polls that number of
   * milliseconds.  The feature is disabled if set to `false`.  It defaults to
   * `false`. */
  keepAlive?: boolean | number;
}

class CloseEvent extends Event {
  constructor(eventInit: EventInit) {
    super("close", eventInit);
  }
}

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 *
 * An event which contains information which will be sent to the remote
 * connection and be made available in an `EventSource` as an event. A server
 * creates new events and dispatches them on the target which will then be
 * sent to a client.
 *
 * See more about Server-sent events on [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events)
 *
 * ### Example
 *
 * ```ts
 * import {
 *   ServerSentEvent,
 *   ServerSentEventStreamTarget,
 * } from "https://deno.land/std@$STD_VERSION/http/server_sent_event.ts";
 *
 * Deno.serve({ port: 8000 }, (request) => {
 *   const target = new ServerSentEventStreamTarget();
 *   const evt = new ServerSentEvent("message", {
 *     data: { hello: "world" },
 *     id: 1
 *   });
 *   target.dispatchEvent(evt);
 *   return target.asResponse();
 * });
 * ```
 */
export class ServerSentEvent extends Event {
  #data: string;
  #id?: number;
  #type: string;

  /**
   * @param type the event type that will be available on the client. The type
   *             of `"message"` will be handled specifically as a message
   *             server-side event.
   * @param eventInit initialization options for the event
   */
  constructor(type: string, eventInit: ServerSentEventInit = {}) {
    super(type, eventInit);
    const { data, replacer, space } = eventInit;
    this.#type = type;
    try {
      this.#data = typeof data === "string"
        ? data
        : data !== undefined
        ? JSON.stringify(data, replacer as (string | number)[], space)
        : "";
    } catch (e) {
      assert(e instanceof Error);
      throw new TypeError(
        `data could not be coerced into a serialized string.\n  ${e.message}`,
      );
    }
    const { id } = eventInit;
    this.#id = id;
  }

  /** The data associated with the event, which will be sent to the client and
   * be made available in the `EventSource`. */
  get data(): string {
    return this.#data;
  }

  /** The optional ID associated with the event that will be sent to the client
   * and be made available in the `EventSource`. */
  get id(): number | undefined {
    return this.#id;
  }

  override toString(): string {
    const data = `data: ${this.#data.split("\n").join("\ndata: ")}\n`;
    return `${this.#type === "__message" ? "" : `event: ${this.#type}\n`}${
      this.#id ? `id: ${String(this.#id)}\n` : ""
    }${data}\n`;
  }
}

const RESPONSE_HEADERS = [
  ["Connection", "Keep-Alive"],
  ["Content-Type", "text/event-stream"],
  ["Cache-Control", "no-cache"],
  ["Keep-Alive", `timeout=${Number.MAX_SAFE_INTEGER}`],
] as const;

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 */
export interface ServerSentEventTarget extends EventTarget {
  /** Is set to `true` if events cannot be sent to the remote connection.
   * Otherwise it is set to `false`.
   *
   * *Note*: This flag is lazily set, and might not reflect a closed state until
   * another event, comment or message is attempted to be processed. */
  readonly closed: boolean;

  /** Close the target, refusing to accept any more events. */
  close(): Promise<void>;

  /** Send a comment to the remote connection.  Comments are not exposed to the
   * client `EventSource` but are used for diagnostics and helping ensure a
   * connection is kept alive.
   *
   * ```ts
   * import { ServerSentEventStreamTarget } from "https://deno.land/std@$STD_VERSION/http/server_sent_event.ts";
   *
   * Deno.serve({ port: 8000 }, (request) => {
   *   const target = new ServerSentEventStreamTarget();
   *   target.dispatchComment("this is a comment");
   *   return target.asResponse();
   * });
   * ```
   */
  dispatchComment(comment: string): boolean;

  /** Dispatch a message to the client.  This message will contain `data: ` only
   * and be available on the client `EventSource` on the `onmessage` or an event
   * listener of type `"message"`. */
  dispatchMessage(data: unknown): boolean;

  /** Dispatch a server sent event to the client.  The event `type` will be
   * sent as `event: ` to the client which will be raised as a `MessageEvent`
   * on the `EventSource` in the client.
   *
   * Any local event handlers will be dispatched to first, and if the event
   * is cancelled, it will not be sent to the client.
   *
   * ```ts
   * import {
   *   ServerSentEvent,
   *   ServerSentEventStreamTarget,
   * } from "https://deno.land/std@$STD_VERSION/http/server_sent_event.ts";
   *
   * Deno.serve({ port: 8000 }, (request) => {
   *   const target = new ServerSentEventStreamTarget();
   *   const evt = new ServerSentEvent("ping", { data: "hello" });
   *   target.dispatchEvent(evt);
   *   return target.asResponse();
   * });
   * ```
   */
  dispatchEvent(event: ServerSentEvent): boolean;

  /** Dispatch a server sent event to the client.  The event `type` will be
   * sent as `event: ` to the client which will be raised as a `MessageEvent`
   * on the `EventSource` in the client.
   *
   * Any local event handlers will be dispatched to first, and if the event
   * is cancelled, it will not be sent to the client.
   *
   * ```ts
   * import {
   *   ServerSentEvent,
   *   ServerSentEventStreamTarget,
   * } from "https://deno.land/std@$STD_VERSION/http/server_sent_event.ts";
   *
   * Deno.serve({ port: 8000 }, (request) => {
   *   const target = new ServerSentEventStreamTarget();
   *   const evt = new ServerSentEvent("ping", { data: "hello" });
   *   target.dispatchEvent(evt);
   *   return target.asResponse();
   * });
   * ```
   */
  dispatchEvent(event: CloseEvent | ErrorEvent): boolean;
}

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 *
 * An implementation of {@linkcode ServerSentEventTarget} that provides a
 * readable stream as a body of a response to establish a connection to a
 * client.
 */
export class ServerSentEventStreamTarget extends EventTarget
  implements ServerSentEventTarget {
  #bodyInit: ReadableStream<Uint8Array>;
  #closed = false;
  #controller?: ReadableStreamDefaultController<Uint8Array>;
  // we are ignoring any here, because when exporting to npm/Node.js, the timer
  // handle isn't a number.
  // deno-lint-ignore no-explicit-any
  #keepAliveId?: any;

  // deno-lint-ignore no-explicit-any
  #error(error: any) {
    this.dispatchEvent(new CloseEvent({ cancelable: false }));
    const errorEvent = new ErrorEvent("error", { error });
    this.dispatchEvent(errorEvent);
  }

  #push(payload: string) {
    if (!this.#controller) {
      this.#error(new Error("The controller has not been set."));
      return;
    }
    if (this.#closed) {
      return;
    }
    this.#controller.enqueue(encoder.encode(payload));
  }

  get closed(): boolean {
    return this.#closed;
  }

  constructor({ keepAlive = false }: ServerSentEventTargetOptions = {}) {
    super();

    this.#bodyInit = new ReadableStream<Uint8Array>({
      start: (controller) => {
        this.#controller = controller;
      },
      cancel: (error) => {
        // connections closing are considered "normal" for SSE events and just
        // mean the far side has closed.
        if (
          error instanceof Error && error.message.includes("connection closed")
        ) {
          this.close();
        } else {
          this.#error(error);
        }
      },
    });

    this.addEventListener("close", () => {
      this.#closed = true;
      if (this.#keepAliveId !== null && this.#keepAliveId !== undefined) {
        clearInterval(this.#keepAliveId);
        this.#keepAliveId = undefined;
      }
      if (this.#controller) {
        try {
          this.#controller.close();
        } catch {
          // we ignore any errors here, as it is likely that the controller
          // is already closed
        }
      }
    });

    if (keepAlive) {
      const interval = typeof keepAlive === "number"
        ? keepAlive
        : DEFAULT_KEEP_ALIVE_INTERVAL;
      this.#keepAliveId = setInterval(() => {
        this.dispatchComment("keep-alive comment");
      }, interval);
    }
  }

  /** Returns a {@linkcode Response} which contains the body and headers needed
   * to initiate a SSE connection with the client. */
  asResponse(responseInit?: ResponseInit): Response {
    return new Response(...this.asResponseInit(responseInit));
  }

  /** Returns a tuple which contains the {@linkcode BodyInit} and
   * {@linkcode ResponseInit} needed to create a response that will establish
   * a SSE connection with the client. */
  asResponseInit(responseInit: ResponseInit = {}): [BodyInit, ResponseInit] {
    const headers = new Headers(responseInit.headers);
    for (const [key, value] of RESPONSE_HEADERS) {
      headers.set(key, value);
    }
    responseInit.headers = headers;
    return [this.#bodyInit, responseInit];
  }

  close(): Promise<void> {
    this.dispatchEvent(new CloseEvent({ cancelable: false }));
    return Promise.resolve();
  }

  dispatchComment(comment: string): boolean {
    this.#push(`: ${comment.split("\n").join("\n: ")}\n\n`);
    return true;
  }

  // deno-lint-ignore no-explicit-any
  dispatchMessage(data: any): boolean {
    const event = new ServerSentEvent("__message", { data });
    return this.dispatchEvent(event);
  }

  override dispatchEvent(event: ServerSentEvent): boolean;
  override dispatchEvent(event: CloseEvent | ErrorEvent): boolean;
  override dispatchEvent(
    event: ServerSentEvent | CloseEvent | ErrorEvent,
  ): boolean {
    const dispatched = super.dispatchEvent(event);
    if (dispatched && event instanceof ServerSentEvent) {
      this.#push(String(event));
    }
    return dispatched;
  }

  [Symbol.for("Deno.customInspect")](inspect: (value: unknown) => string) {
    return `${this.constructor.name} ${
      inspect({ "#bodyInit": this.#bodyInit, "#closed": this.#closed })
    }`;
  }

  [Symbol.for("nodejs.util.inspect.custom")](
    depth: number,
    // deno-lint-ignore no-explicit-any
    options: any,
    inspect: (value: unknown, options?: unknown) => string,
  ) {
    if (depth < 0) {
      return options.stylize(`[${this.constructor.name}]`, "special");
    }

    const newOptions = Object.assign({}, options, {
      depth: options.depth === null ? null : options.depth - 1,
    });
    return `${options.stylize(this.constructor.name, "special")} ${
      inspect(
        { "#bodyInit": this.#bodyInit, "#closed": this.#closed },
        newOptions,
      )
    }`;
  }
}
