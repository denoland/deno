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

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 */
import {
  ServerSentEvent as ServerSentEvent_,
  type ServerSentEventInit as ServerSentEventInit_,
  ServerSentEventStreamTarget as ServerSentEventStreamTarget_,
  type ServerSentEventTarget as ServerSentEventTarget_,
  type ServerSentEventTargetOptions as ServerSentEventTargetOptions_,
} from "./unstable_server_sent_event.ts";

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 */
export type ServerSentEventInit = ServerSentEventInit_;
/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 */
export type ServerSentEventTarget = ServerSentEventTarget_;
/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 */
export type ServerSentEventTargetOptions = ServerSentEventTargetOptions_;

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
export const ServerSentEvent = ServerSentEvent_;

/**
 * @deprecated (will be removed in 0.209.0) Use {@linkcode ServerSentEventStream} from {@link https://deno.land/std/http/server_sent_event_stream.ts} instead.
 *
 * An implementation of {@linkcode ServerSentEventTarget} that provides a
 * readable stream as a body of a response to establish a connection to a
 * client.
 */
export const ServerSentEventStreamTarget = ServerSentEventStreamTarget_;
