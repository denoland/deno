// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

const core = globalThis.Deno.core;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { URL } from "ext:deno_url/00_url.js";
import DOMException from "ext:deno_web/01_dom_exception.js";
import {
  defineEventHandler,
  EventTarget,
  setIsTrusted,
} from "ext:deno_web/02_event.js";
import { TransformStream } from "ext:deno_web/06_streams.js";
import { TextDecoderStream } from "ext:deno_web/08_text_encoding.js";
import { getLocationHref } from "ext:deno_web/12_location.js";
import { newInnerRequest } from "ext:deno_fetch/23_request.js";
import { mainFetch } from "ext:deno_fetch/26_fetch.js";

const primordials = globalThis.__bootstrap.primordials;
const {
  JSONStringify,
  Number,
  NumberIsFinite,
  NumberIsNaN,
  ObjectDefineProperties,
  Promise,
  Symbol,
} = primordials;

// Copied from https://github.com/denoland/deno_std/blob/e0753abe0c8602552862a568348c046996709521/streams/text_line_stream.ts#L20-L74
export class TextLineStream extends TransformStream {
  #allowCR;
  #buf = "";

  constructor(options) {
    super({
      transform: (chunk, controller) => {
        console.error("chunk tls:", JSONStringify(chunk));
        return this.#handle(chunk, controller);
      },
      flush: (controller) => {
        if (this.#buf.length > 0) {
          if (
            this.#allowCR &&
            this.#buf[this.#buf.length - 1] === "\r"
          ) controller.enqueue(this.#buf.slice(0, -1));
          else controller.enqueue(this.#buf);
        }
      },
    });
    this.#allowCR = options?.allowCR ?? false;
  }

  #handle(chunk, controller) {
    chunk = this.#buf + chunk;

    for (;;) {
      const lfIndex = chunk.indexOf("\n");

      if (this.#allowCR) {
        const crIndex = chunk.indexOf("\r");

        if (
          crIndex !== -1 && crIndex !== (chunk.length - 1) &&
          (lfIndex === -1 || (lfIndex - 1) > crIndex)
        ) {
          console.error("enqueue1:", chunk.slice(0, crIndex));
          controller.enqueue(chunk.slice(0, crIndex));
          chunk = chunk.slice(crIndex + 1);
          continue;
        }
      }

      if (lfIndex !== -1) {
        let crOrLfIndex = lfIndex;
        if (chunk[lfIndex - 1] === "\r") {
          crOrLfIndex--;
        }
        controller.enqueue(chunk.slice(0, crOrLfIndex));
        chunk = chunk.slice(lfIndex + 1);
        continue;
      }

      break;
    }

    this.#buf = chunk;
  }
}

const CONNECTING = 0;
const OPEN = 1;
const CLOSED = 2;

const _url = Symbol("[[url]]");
const _withCredentials = Symbol("[[withCredentials]]");
const _readyState = Symbol("[[readyState]]");
const _reconnectionTime = Symbol("[[reconnectionTime]]");
const _lastEventID = Symbol("[[lastEventID]]");
const _abortController = Symbol("[[abortController]]");
const _loop = Symbol("[[loop]]");

class EventSource extends EventTarget {
  /** @type {AbortController} */
  [_abortController] = new AbortController();

  /** @type {number} */
  [_reconnectionTime] = 5000;

  /** @type {string} */
  [_lastEventID] = "";

  /** @type {number} */
  [_readyState] = CONNECTING;
  get readyState() {
    webidl.assertBranded(this, EventSourcePrototype);
    return this[_readyState];
  }

  get CONNECTING() {
    webidl.assertBranded(this, EventSourcePrototype);
    return CONNECTING;
  }
  get OPEN() {
    webidl.assertBranded(this, EventSourcePrototype);
    return OPEN;
  }
  get CLOSED() {
    webidl.assertBranded(this, EventSourcePrototype);
    return CLOSED;
  }

  /** @type {string} */
  [_url];
  get url() {
    webidl.assertBranded(this, EventSourcePrototype);
    return this[_url];
  }

  /** @type {boolean} */
  [_withCredentials];
  get withCredentials() {
    webidl.assertBranded(this, EventSourcePrototype);
    return this[_withCredentials];
  }

  constructor(url, eventSourceInitDict = {}) {
    super();
    this[webidl.brand] = webidl.brand;
    const prefix = "Failed to construct 'EventSource'";
    webidl.requiredArguments(arguments.length, 1, {
      prefix,
    });
    url = webidl.converters.USVString(url, {
      prefix,
      context: "Argument 1",
    });
    eventSourceInitDict = webidl.converters.EventSourceInit(
      eventSourceInitDict,
      {
        prefix,
        context: "Argument 2",
      },
    );

    try {
      url = new URL(url, getLocationHref()).href;
    } catch (e) {
      throw new DOMException(e.message, "SyntaxError");
    }

    this[_url] = url;
    this[_withCredentials] = eventSourceInitDict.withCredentials;

    this[_loop]();
  }

  close() {
    webidl.assertBranded(this, EventSourcePrototype);
    this[_abortController].abort();
    this[_readyState] = CLOSED;
  }

  async [_loop]() {
    let lastEventIDValue = "";
    let lastEventID = "";
    while (this[_readyState] !== CLOSED) {
      const req = newInnerRequest(
        "GET",
        this[_url],
        () =>
          lastEventIDValue === ""
            ? [
              ["accept", "text/event-stream"],
            ]
            : [
              ["accept", "text/event-stream"],
              [
                "last-event-id",
                core.ops.op_utf8_to_byte_string(lastEventIDValue),
              ],
            ],
        null,
        false,
      );
      /** @type {InnerResponse} */
      const res = await mainFetch(req, true, this[_abortController].signal);

      const contentType = res.headerList.find((header) =>
        header[0].toLowerCase() === "content-type"
      );
      if (res.type === "error") {
        if (res.aborted) {
          this[_readyState] = CLOSED;
          this.dispatchEvent(new Event("error"));
          break;
        } else {
          if (this[_readyState] === CLOSED) {
            this[_abortController].abort();
            break;
          }
          this[_readyState] = CONNECTING;
          this.dispatchEvent(new Event("error"));
          await new Promise((res) => setTimeout(res, this[_reconnectionTime]));
          if (this[_readyState] !== CONNECTING) {
            continue;
          }

          if (this[_lastEventID] !== "") {
            lastEventIDValue = this[_lastEventID];
          }
          continue;
        }
      } else if (
        res.status !== 200 ||
        !contentType?.[1].toLowerCase().includes("text/event-stream")
      ) {
        this[_readyState] = CLOSED;
        this.dispatchEvent(new Event("error"));
        break;
      }

      if (this[_readyState] !== CLOSED) {
        this[_readyState] = OPEN;
        this.dispatchEvent(new Event("open"));

        let data = "";
        let eventType = "";
        for await (
          const chunk of res.body.stream
            .pipeThrough(new TextDecoderStream())
            .pipeThrough(new TextLineStream())
        ) {
          console.error("chunk:", JSONStringify(chunk));
          if (chunk === "") {
            this[_lastEventID] = lastEventID;
            if (data === "") {
              eventType = "";
              continue;
            }
            if (data.endsWith("\n")) {
              data = data.slice(0, -1);
            }
            const event = new MessageEvent(eventType || "message", {
              data,
              origin: res.url(),
              lastEventId: this[_lastEventID],
            });
            setIsTrusted(event, true);
            data = "";
            eventType = "";
            if (this[_readyState] !== CLOSED) {
              this.dispatchEvent(event);
            }
          } else if (chunk.startsWith(":")) {
            continue;
          } else {
            let field = chunk;
            let value = "";
            if (chunk.includes(":")) {
              ({ 0: field, 1: value } = chunk.split(":"));
              if (value.startsWith(" ")) {
                value = value.slice(1);
              }
            }

            switch (field) {
              case "event": {
                eventType = value;
                break;
              }
              case "data": {
                data += value + "\n";
                break;
              }
              case "id": {
                if (!value.includes("\0")) {
                  console.error("new id:", JSONStringify(value));
                  lastEventID = value;
                }
                break;
              }
              case "retry": {
                const reconnectionTime = Number(value);
                if (
                  !NumberIsNaN(reconnectionTime) &&
                  NumberIsFinite(reconnectionTime)
                ) {
                  this[_reconnectionTime] = reconnectionTime;
                }
                break;
              }
            }
          }

          if (this[_abortController].signal.aborted) {
            break;
          }

          // TODO: Once the end of the file is reached, any pending data must be discarded. (If the file ends in the middle of an event, before the final empty line, the incomplete event is not dispatched.)
        }
        if (this[_readyState] === CLOSED) {
          this[_abortController].abort();
          break;
        }
        this[_readyState] = CONNECTING;
        this.dispatchEvent(new Event("error"));
        await new Promise((res) => setTimeout(res, this[_reconnectionTime]));
        if (this[_readyState] !== CONNECTING) {
          continue;
        }

        if (this[_lastEventID] !== "") {
          lastEventIDValue = this[_lastEventID];
        }
      }
    }
  }
}

const EventSourcePrototype = EventSource.prototype;

ObjectDefineProperties(EventSource, {
  CONNECTING: {
    value: 0,
  },
  OPEN: {
    value: 1,
  },
  CLOSED: {
    value: 2,
  },
});

defineEventHandler(EventSource.prototype, "open");
defineEventHandler(EventSource.prototype, "message");
defineEventHandler(EventSource.prototype, "error");

webidl.converters.EventSourceInit = webidl.createDictionaryConverter(
  "EventSourceInit",
  [
    {
      key: "withCredentials",
      defaultValue: false,
      converter: webidl.converters.boolean,
    },
  ],
);

export { EventSource };
