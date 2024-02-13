// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

const NEWLINE_REGEXP = /\r\n|\r|\n/;
const encoder = new TextEncoder();

/**
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#fields}
 */
export interface ServerSentEventMessage {
  /** Ignored by the client. */
  comment?: string;
  /** A string identifying the type of event described. */
  event?: string;
  /** The data field for the message. Split by new lines. */
  data?: string;
  /** The event ID to set the {@linkcode EventSource} object's last event ID value. */
  id?: string | number;
  /** The reconnection time. */
  retry?: number;
}

function assertHasNoNewline(value: string, varName: string) {
  if (value.match(NEWLINE_REGEXP) !== null) {
    throw new RangeError(`${varName} cannot contain a newline`);
  }
}

/**
 * Converts a server-sent message object into a string for the client.
 *
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#event_stream_format}
 */
function stringify(message: ServerSentEventMessage): Uint8Array {
  const lines = [];
  if (message.comment) {
    assertHasNoNewline(message.comment, "`message.comment`");
    lines.push(`:${message.comment}`);
  }
  if (message.event) {
    assertHasNoNewline(message.event, "`message.event`");
    lines.push(`event:${message.event}`);
  }
  if (message.data) {
    message.data.split(NEWLINE_REGEXP).forEach((line) =>
      lines.push(`data:${line}`)
    );
  }
  if (message.id) {
    assertHasNoNewline(message.id.toString(), "`message.id`");
    lines.push(`id:${message.id}`);
  }
  if (message.retry) lines.push(`retry:${message.retry}`);
  return encoder.encode(lines.join("\n") + "\n\n");
}

/**
 * Transforms server-sent message objects into strings for the client.
 *
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events}
 *
 * @example
 * ```ts
 * import {
 *   type ServerSentEventMessage,
 *   ServerSentEventStream,
 * } from "https://deno.land/std@$STD_VERSION/http/server_sent_event_stream.ts";
 *
 * const stream = ReadableStream.from<ServerSentEventMessage>([
 *   { data: "hello there" }
 * ]).pipeThrough(new ServerSentEventStream());
 * new Response(stream, {
 *   headers: {
 *     "content-type": "text/event-stream",
 *     "cache-control": "no-cache",
 *   },
 * });
 * ```
 */
export class ServerSentEventStream
  extends TransformStream<ServerSentEventMessage, Uint8Array> {
  constructor() {
    super({
      transform: (message, controller) => {
        controller.enqueue(stringify(message));
      },
    });
  }
}
