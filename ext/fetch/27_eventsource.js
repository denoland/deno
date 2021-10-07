// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// Refer https://github.com/MierenManz/EventSource
((window) => {
  const { webidl } = window.__bootstrap;
  const { DOMException } = window.__bootstrap.domException;
  const { defineEventHandler } = window.__bootstrap.webUtil;
  const { ObjectDefineProperties, Symbol } = window.__bootstrap.primordials;

  webidl.converters["EventSourceInit"] = webidl.createDictionaryConverter(
    "EventSourceInit",
    [
      {
        key: "withCredentials",
        converter: webidl.converters.boolean,
        required: true,
      },
    ],
  );

  const CONNECTING = 0;
  const OPEN = 1;
  const CLOSED = 2;
  const defaultFetchSettings = {
    headers: [["Accept", "text/event-stream"]],
    credentials: "same-origin",
    mode: "cors",
  }

  const _readyState = Symbol("readystate");
  const _withCredentials = Symbol("withcredentials");
  const _abortController = Symbol("abortcontroller");
  const _url = Symbol("url");
  const _fetchSettings = Symbol("fetchsettings");
  const _lastEventID = Symbol("lasteventid");
  const _reconnectionTime = Symbol("reconnetiontime");

  class EventSource extends EventTarget {
    [_readyState] = CONNECTING;
    [_withCredentials] = false;
    [_abortController] = new AbortController();
    [_url] = "";
    [_fetchSettings] = defaultFetchSettings;
    [_lastEventID] = "";
    [_reconnectionTime] = 2200;
    get CONNECTING() {
      webidl.assertBranded(this, EventSource);
      return CONNECTING;
    }

    get OPEN() {
      webidl.assertBranded(this, EventSource);
      return OPEN;
    }

    get CLOSED() {
      webidl.assertBranded(this, EventSource);
      return CLOSED;
    }

    get readyState() {
      webidl.assertBranded(this, EventSource);
      return this[_readyState];
    }

    get url() {
      webidl.assertBranded(this, EventSource);
      return this[_url];
    }

    get withCredentials() {
      webidl.assertBranded(this, EventSource);
      return this[_withCredentials];
    }

    constructor(url, eventSourceInitDict) {
      super();

      const prefix = "Failed to construct 'EventSource'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      if (eventSourceInitDict) {
        eventSourceInitDict = webidl.converters["EventSourceInit"]({
          prefix,
          context: "Argument 2",
        });
      }

      try {
        // Allow empty url
        // https://github.com/web-platform-tests/wpt/blob/master/eventsource/eventsource-constructor-empty-url.any.js
        this[_url] = url == ""
          ? window.location.toString()
          : new URL(url, window.location.href).toString();
      } catch (e) {
        // Dunno if this is allowd in the spec. But handy for testing purposes
        if (e instanceof ReferenceError) {
          this[_url] = new URL(url).toString();
        } else throw new DOMException(e.message, "SyntaxError");
      }

      if (eventSourceInitDict?.withCredentials) {
        this[_fetchSettings].credentials = "include";
        this[_withCredentials] = true;
      }

      this.#fetch();
      return;
    }

    close() {
      webidl.assertBranded(this, EventSource);
      this[_readyState] = CLOSED;
      this[_abortController].abort();
    }

    async #fetch() {
      let currentRetries = 0;
      while (this[_readyState] < CLOSED) {
        const res = await fetch(this[_url], {
          cache: "no-store",
          // This seems to cause problems if the abort happens while `res.body` is being used
          // signal: this[_abortController].signal,
          keepalive: true,
          redirect: "follow",
          ...this[_fetchSettings],
        }).catch(() => void (0));

        if (
          res?.body &&
          res?.status === 200 &&
          res.headers.get("content-type")?.startsWith("text/event-stream")
        ) {
          // Announce connection
          if (this[_readyState] !== CLOSED) {
            this[_readyState] = OPEN;
            const openEvent = new Event("open", {
              bubbles: false,
              cancelable: false,
            });
            super.dispatchEvent(openEvent);
            if (this.onopen) this.onopen(openEvent);
          }

          // Decode body for interpreting
          const decoder = new TextDecoderStream("utf-8", {
            ignoreBOM: false,
            fatal: false,
          });
          const reader = res.body.pipeThrough(decoder);

          // Initiate buffers
          let lastEventIDBuffer = "";
          let eventTypeBuffer = "";
          let messageBuffer = "";
          let readBuffer = "";

          for await (const chunk of reader) {
            if (this[_abortController].signal.aborted) break;
            const lines = decodeURIComponent(readBuffer + chunk)
              .replaceAll("\r\n", "\n")
              .replaceAll("\r", "\n")
              .split("\n");
            readBuffer = lines.pop() ?? "";

            // Start loop for interpreting
            for (const line of lines) {
              if (!line) {
                this.#settings.lastEventID = lastEventIDBuffer;

                // Check if buffer is not an empty string
                if (messageBuffer) {
                  // Create event
                  if (!eventTypeBuffer) {
                    eventTypeBuffer = "message";
                  }

                  const event = new MessageEvent(eventTypeBuffer, {
                    data: messageBuffer.trim(),
                    origin: res.url,
                    lastEventId: this[_lastEventID],
                    cancelable: false,
                    bubbles: false,
                  });

                  if (this[_readyState] !== CLOSED) {
                    // Fire event
                    super.dispatchEvent(event);
                    if (this.onmessage) this.onmessage(event);
                  }
                }

                // Clear buffers
                messageBuffer = "";
                eventTypeBuffer = "";
                continue;
              }

              // Ignore comments
              if (line[0] === ":") continue;

              let splitIndex = line.indexOf(":");
              splitIndex = splitIndex > 0 ? splitIndex : line.length;
              const field = line.slice(0, splitIndex).trim();
              const data = line.slice(splitIndex + 1).trim();
              switch (field) {
                case "event":
                  // Set fieldBuffer to Field Value
                  eventTypeBuffer = data;
                  break;
                case "data":
                  // append Field Value to dataBuffer
                  messageBuffer += `${data}\n`;
                  break;
                case "id":
                  // set lastEventID to Field Value
                  if (
                    data && !data.includes("\u0000") && !data.includes("\x00")
                  ) {
                    lastEventIDBuffer = data;
                  }
                  break;
                case "retry": {
                  // set reconnectionTime to Field Value if int
                  const num = Number(data);
                  if (!isNaN(num) && isFinite(num)) {
                    this[_reconnectionTime] = num;
                  }
                  break;
                }
              }
            }
          }
          if (this[_abortController].signal.aborted) {
            // Cancel reader to close the EventSource properly
            await reader.cancel();
            this[_readyState] = CLOSED;
            break;
          }
        } else {
          // Connection failed for whatever reason
          this[_readyState] = CLOSED;
          this[_abortController].abort();
          const errorEvent = new Event("error", {
            bubbles: false,
            cancelable: false,
          });
          super.dispatchEvent(errorEvent);
          if (this.onerror) this.onerror(errorEvent);
          if (currentRetries >= 3) break;
          currentRetries++;
        }

        // Set readyState to CONNECTING
        if (this[_readyState] !== CLOSED) {
          this[_readyState] = CONNECTING;

          // Fire onerror
          const errorEvent = new Event("error", {
            bubbles: false,
            cancelable: false,
          });

          super.dispatchEvent(errorEvent);
          if (this.onerror) this.onerror(errorEvent);

          // Timeout for re-establishing the connection
          await new Promise((res) => {
            const id = setTimeout(
              () => res(clearTimeout(id)),
              this[_reconnectionTime],
            );
          });

          if (this[_readyState] !== CONNECTING) break;

          if (this[_lastEventID]) {
            this[_fetchSettings].headers.push([
              "Last-Event-ID",
              this[_lastEventID],
            ]);
          }
        }
      }
    }
  }

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

  defineEventHandler(EventSource.prototype, "message");
  defineEventHandler(EventSource.prototype, "error");
  defineEventHandler(EventSource.prototype, "open");
  webidl.configurePrototype(EventSource);

  window.__bootstrap.eventSource = EventSource;
})(this);
