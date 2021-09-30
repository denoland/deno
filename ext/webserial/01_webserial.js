// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { DOMException } = window.__bootstrap.domException;
  const {
    Promise,
    PromiseResolve,
    PromiseAll,
    Symbol,
    TypeError,
    Uint8Array,
  } = window.__bootstrap.primordials;

  webidl.converters.SerialPortRequestOptions = webidl.createDictionaryConverter(
    "SerialPortRequestOptions",
    [
      {
        key: "filters",
        converter: webidl.createSequenceConverter(
          webidl.converters.SerialPortFilter,
        ),
      },
    ],
  );

  webidl.converters.SerialPortFilter = webidl.createDictionaryConverter(
    "SerialPortFilter",
    [
      {
        key: "usbVendorId",
        converter: webidl.converters["unsigned short"],
      },
      {
        key: "usbProductId",
        converter: webidl.converters["unsigned short"],
      },
    ],
  );

  webidl.converters.ParityType = webidl.createEnumConverter("ParityType", [
    "none",
    "even",
    "odd",
  ]);

  webidl.converters.FlowControlType = webidl.createEnumConverter(
    "FlowControlType",
    [
      "none",
      "hardware",
    ],
  );

  webidl.converters.SerialOptions = webidl.createDictionaryConverter(
    "SerialOptions",
    [
      {
        key: "baudRate",
        converter: (V, opts) =>
          webidl.converters["unsigned long"](V, {
            ...opts,
            enforceRange: true,
          }),
        required: true,
      },
      {
        key: "dataBits",
        converter: (V, opts) =>
          webidl.converters.octet(V, {
            ...opts,
            enforceRange: true,
          }),
        defaultValue: 8,
      },
      {
        key: "stopBits",
        converter: (V, opts) =>
          webidl.converters.octet(V, {
            ...opts,
            enforceRange: true,
          }),
        defaultValue: 1,
      },
      {
        key: "parity",
        converter: webidl.converters.ParityType,
        defaultValue: "none",
      },
      {
        key: "bufferSize",
        converter: (V, opts) =>
          webidl.converters["unsigned long"](V, {
            ...opts,
            enforceRange: true,
          }),
        defaultValue: 255,
      },
      {
        key: "flowControl",
        converter: webidl.converters.FlowControlType,
        defaultValue: "none",
      },
    ],
  );

  webidl.converters.SerialOutputSignals = webidl.createDictionaryConverter(
    "SerialOutputSignals",
    [
      {
        key: "dataTerminalReady",
        converter: webidl.converters.boolean,
      },
      {
        key: "requestToSend",
        converter: webidl.converters.boolean,
      },
      {
        key: "break",
        converter: webidl.converters.boolean,
      },
    ],
  );

  class Serial {
    constructor() {
      webidl.illegalConstructor();
    }

    // deno-lint-ignore require-await
    async getPorts() {
      webidl.assertBranded(this, Serial);
      const ports = core.opSync("op_webserial_get_ports");

      // TODO(@crowlKats): maybe cache ports?
      return ports.map((port) => {
        const serial = webidl.createBranded(SerialPort);
        serial[_name] = port.name;
        serial[_info] = {
          usbVendorId: info?.[0],
          usbProductId: info?.[1],
        };
        return serial;
      });
    }

    /*async requestPort(options = {}) {
      webidl.assertBranded(this, Serial);
      const prefix = "Failed to execute 'requestPort' on 'Serial'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      options = webidl.converters.SerialPortRequestOptions(options, {
        prefix,
        context: "Argument 1",
      });

      if (options.filters) {
        for (const filter of options.filters) {
          if (!("usbVendorId" in filter)) {
            throw TypeError();
          }
        }
      }
    }*/
  }

  const _state = Symbol("[[state]]");
  const _bufferSize = Symbol("[[bufferSize]]");
  const _readable = Symbol("[[readable]]");
  const _readFatal = Symbol("[[readFatal]]");
  const _writable = Symbol("[[writable]]");
  const _writeFatal = Symbol("[[writeFatal]]");
  const _pendingClosePromise = Symbol("[[pendingClosePromise]]");
  const _rid = Symbol("[[rid]]");
  const _name = Symbol("[[name]]");
  const _info = Symbol("[[info]]");

  class SerialPort {
    [_state] = "closed";
    [_bufferSize] = undefined;
    [_readable] = null;
    [_readFatal] = false;
    [_writable] = null;
    [_writeFatal] = false;
    [_pendingClosePromise] = null;

    [_rid];
    [_name];
    [_info];

    get readable() {
      webidl.assertBranded(this, SerialPort);
      if (this[_readable] !== null) {
        return this[_readable];
      }
      if (this[_state] !== "opened") {
        return null;
      }
      if (this[_readFatal]) {
        return null;
      }

      const stream = new ReadableStream({
        pull: async (controller) => {
          const buf = new Uint8Array(controller.desiredSize);
          await core.opAsync("op_webserial_read", this[_rid], buf); // TODO(@crowlKats): errors
          controller.enqueue(buf);
        },
        cancel: () => {
          // TODO(@crowlKats): Invoke the operating system to discard the contents of all software and hardware receive buffers for the port.
          this.#handleClosingReadable();
        },
      }, {
        highWaterMark: this[_bufferSize],
        size: (chunk) => chunk.byteLength,
      });

      this[_readable] = stream;
      return stream;
    }

    #handleClosingReadable() {
      this[_readable] = null;
      if (this[_writable] === null && this[_pendingClosePromise] !== null) {
        this[_pendingClosePromise].resolve(undefined);
      }
    }

    get writable() {
      webidl.assertBranded(this, SerialPort);
      if (this[_writable] !== null) {
        return this[_writable];
      }
      if (this[_state] !== "opened") {
        return null;
      }
      if (this[_writeFatal]) {
        return null;
      }

      const stream = new WritableStream({
        write: async (chunk) => {
          const source = webidl.converters.BufferSource(chunk);
          await core.opAsync("op_webserial_write", this[_rid], source.slice()); // TODO(@crowlKats): errors
        },
        abort: () => {
          // TODO(@crowlKats): Invoke the operating system to discard the contents of all software and hardware transmit buffers for the port.
          this.#handleClosingWritable();
        },
        close: () => {
          // TODO(@crowlKats): Invoke the operating system to flush the contents of all software and hardware transmit buffers for the port.
          this.#handleClosingWritable();
        },
      }, {
        highWaterMark: this[_bufferSize],
        size: (chunk) => chunk.byteLength,
      });

      this[_writable] = stream;
      return stream;
    }

    #handleClosingWritable() {
      this[_writable] = null;
      if (this[_readable] === null && this[_pendingClosePromise] !== null) {
        this[_pendingClosePromise].resolve(undefined);
      }
    }

    constructor() {
      webidl.illegalConstructor();
    }

    getInfo() {
      webidl.assertBranded(this, SerialPort);
      return this[_info];
    }

    // deno-lint-ignore require-await
    async open(options) {
      webidl.assertBranded(this, SerialPort);
      const prefix = "Failed to execute 'open' on 'SerialPort'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      options = webidl.converters.SerialOptions(options, {
        prefix,
        context: "Argument 1",
      });

      if (this[_state] !== "closed") {
        throw new DOMException(
          "SerialPort must be closed",
          "InvalidStateError",
        );
      }

      if (options.dataBits !== 7 || options.dataBits !== 8) {
        throw new TypeError("Invalid 'dataBits' given, must be either 7 or 8.");
      }
      if (options.stopBits !== 1 || options.stopBits !== 2) {
        throw new TypeError("Invalid 'stopBits' given, must be either 1 or 2.");
      }
      if (options.bufferSize === 0) {
        throw new TypeError(
          "Invalid 'bufferSize' given, must be greater than 0.",
        );
      }

      this[_state] = "opening";
      this[_rid] = core.opSync("op_webserial_open_port", this[_name], options);
      this[_state] = "opened";
      this[_bufferSize] = options.bufferSize;
    }

    async setSignals(options = {}) {
      webidl.assertBranded(this, SerialPort);
      const prefix = "Failed to execute 'setSignals' on 'SerialPort'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      options = webidl.converters.SerialPortRequestOptions(options, {
        prefix,
        context: "Argument 1",
      });

      if (this[_state] !== "opened") {
        throw new DOMException(
          "SerialPort must be opened",
          "InvalidStateError",
        );
      }

      // TODO(@crowlKats): If all of the specified members of signals are not present reject promise with TypeError and return promise.
      await core.opAsync("op_webserial_set_signals", this[_rid], options);
    }

    async getSignals() {
      webidl.assertBranded(this, SerialPort);
      if (this[_state] !== "opened") {
        throw new DOMException(
          "SerialPort must be opened",
          "InvalidStateError",
        );
      }

      return await core.opAsync("op_webserial_get_signals", this[_rid]);
    }

    async close() {
      webidl.assertBranded(this, SerialPort);
      let cancelPromise;
      if (this[_readable] === null) {
        cancelPromise = PromiseResolve(undefined);
      } else {
        cancelPromise = this[_readable].cancel();
      }

      let abortPromise;
      if (this[_writable] === null) {
        abortPromise = PromiseResolve(undefined);
      } else {
        abortPromise = this[_writable].abort();
      }

      const pendingClosePromise = new Promise((res) => {
        if (this[_readable] === null && this[_writable] === null) {
          res(undefined);
        }
      });
      // TODO(@crowlKats): check
      this[_pendingClosePromise] = pendingClosePromise;

      const combinedPromise = PromiseAll([
        cancelPromise,
        abortPromise,
        pendingClosePromise,
      ]);
      this[_state] = "closing";
      try {
        await combinedPromise;
        core.close(this[_rid]); // TODO(@crowlKats): check
        this[_state] = "closed";
        this[_readFatal] = false;
        this[_writeFatal] = false;
        this[_pendingClosePromise] = null;
      } catch (e) {
        this[_pendingClosePromise] = null;
        throw e;
      }
    }
  }

  window.__bootstrap.webSerial = {
    serial: webidl.createBranded(Serial),
    Serial,
    SerialPort,
  };
})(this);
