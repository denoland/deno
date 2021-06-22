// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

  class UsbConfiguration {
    #name;
    #value;
    #interfaces;
    constructor({ configurationValue, configurationName, interfaces }) {
      this.#name = configurationName;
      this.#value = configurationValue;
      this.#interfaces = interfaces;
    }

    get configurationName() {
      return this.#name;
    }

    get configurationValue() {
      return this.#value;
    }

    get interfaces() {
      return this.#interfaces;
    }
  }

  // https://wicg.github.io/webusb/#check-the-validity-of-the-control-transfer-parameters
  function validateControlSetup(configuration, setup) {
    if (configuration) {
      // 4.
      if (setup.recipient == "interface") {
        // 1.
        const interfaceNumber = setup.index & 0xFF;
        // 2.
        const _interface = configuration.interfaces.find((itf) =>
          itf.interfaceNumber == interfaceNumber
        );

        if (!_interface) {
          throw new DOMException(
            "Interface does not exist in active configuration.",
            "NotFoundError",
          );
        }

        // 3.
        if (!_interface.claimed) {
          throw new DOMException(
            "Interface must be claimed first.",
            "InvalidStateError",
          );
        }
      } // 5.
      else if (setup.recipient == "endpoint") {
        // 1.
        const endpointNumber = setup.index & (1 << 4);

        // 2.
        const direction = ((setup.index >>> 8) & 1) == 1 ? "in" : "out";

        // 3.
        const _interface = configuration.interfaces.find((itf) =>
          itf.alternates.find((alt) =>
            alt.endpoints.find((ep) => {
              return endpointNumber == ep.endpointNumber &&
                direction == ep.direction;
            })
          )
        );

        if (!_interface) {
          throw new DOMException(
            "Interface does not exist in active configuration.",
            "NotFoundError",
          );
        }

        // 4.
        if (!_interface.claimed) {
          throw new DOMException(
            "Interface must be claimed first.",
            "InvalidStateError",
          );
        }
      }
    }
  }

  const USBDeviceDictionary = [
    {
      key: "usbVersionMajor",
      converter: webidl.converters["octet"],
    },
    {
      key: "usbVersionMinor",
      converter: webidl.converters["octet"],
    },
    {
      key: "usbVersionSubminor",
      converter: webidl.converters["octet"],
    },
    {
      key: "deviceClass",
      converter: webidl.converters["octet"],
    },
    {
      key: "deviceSubclass",
      converter: webidl.converters["octet"],
    },
    {
      key: "deviceProtocol",
      converter: webidl.converters["octet"],
    },
    {
      key: "vendorId",
      converter: webidl.converters["unsigned short"],
    },
    {
      key: "productId",
      converter: webidl.converters["unsigned short"],
    },
    {
      key: "deviceVersionMajor",
      converter: webidl.converters["octet"],
    },
    {
      key: "deviceVersionMinor",
      converter: webidl.converters["octet"],
    },
    {
      key: "deviceVersionSubminor",
      converter: webidl.converters["octet"],
    },
    {
      key: "manufacturerName",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "productName",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "serialNumber",
      converter: webidl.converters["DOMString"],
    },
    {
      key: "configuration",
      converter: webidl.converters["USBConfiguration"],
    },
    {
      key: "configurations",
      converter: webidl.converters["seqeunce<USBConfiguration>"],
    },
  ];

  function mixinDevice(prototype, deviceSymbol) {
    for (const idx in USBDeviceDictionary) {
      const key = USBDeviceDictionary[idx].key;
      Object.defineProperty(prototype.prototype, key, {
        get() {
          webidl.assertBranded(this, prototype);
          if (this[deviceSymbol] == null) {
            return null;
          } else {
            return this[deviceSymbol][key];
          }
        },
        configurable: true,
        enumerable: true,
      });
    }
  }

  const _device = Symbol("device");
  const _rid = Symbol("rid");
  const _handle = Symbol("handle");

  class UsbDevice {
    // Represents a `USBDevice` in closed state. It is used to open a device.
    [_rid];
    // Represents a `USBDeviceHandle`. The actual device handle.
    // Should always be non-null when `this.open` is true.
    [_handle] = null;
    // `USBDevice` properties.
    [_device];

    constructor() {
      webidl.illegalConstructor();
    }

    async claimInterface(interfaceNumber) {
      const prefix = "Failed to execute 'claimInterface' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      interfaceNumber = webidl.converters["octet"](interfaceNumber, {
        prefix,
        context: "Argument 1",
      });

      // 2.
      const _interface = this.configuration.interfaces.find((itf) =>
        itf.interfaceNumber == interfaceNumber
      );
      if (!_interface) {
        throw new DOMException(
          "Interface does not exist in active configuration.",
          "NotFoundError",
        );
      }

      // 3.
      if (!this.opened) {
        throw new DOMException(
          "The device must be opened first.",
          "InvalidStateError",
        );
      }

      // 4.
      if (!_interface.claimed) {
        if (!usbTest[_initialized]) {
          // 5.
          await core.opAsync(
            "op_webusb_claim_interface",
            { rid: this[_handle], interfaceNumber },
          );
        }
        // 6.
        _interface.claimed = true;
      }
    }

    async releaseInterface(interfaceNumber) {
      const prefix = "Failed to execute 'releaseInterface' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      interfaceNumber = webidl.converters["octet"](interfaceNumber, {
        prefix,
        context: "Argument 1",
      });

      // 3.
      const _interface = this.configuration.interfaces.find((itf) =>
        itf.interfaceNumber == interfaceNumber
      );
      if (!_interface) {
        throw new DOMException(
          "Interface does not exist in active configuration.",
          "NotFoundError",
        );
      }

      // 4.
      if (!this.opened) {
        throw new DOMException(
          "The device must be opened first.",
          "InvalidStateError",
        );
      }

      // 5.
      if (_interface.claimed) {
        // 6.
        if (!usbTest[_initialized]) {
          await core.opAsync(
            "op_webusb_release_interface",
            { rid: this[_handle], interfaceNumber },
          );
        }
        // 7.
        _interface.claimed = false;
      }
    }

    async selectConfiguration(configurationValue) {
      const prefix = "Failed to execute 'selectConfiguration' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      configurationValue = webidl.converters["octet"](configurationValue, {
        prefix,
        context: "Argument 1",
      });

      // 3.
      const configuration = this.configurations.find((cnf) =>
        cnf.configurationValue == configurationValue
      );
      if (!configuration) {
        throw new DOMException(
          "Device configuration does not exist.",
          "NotFoundError",
        );
      }

      // 4.
      if (!this.opened) {
        throw new DOMException(
          "The device must be opened first.",
          "InvalidStateError",
        );
      }

      if (!usbTest[_initialized]) {
        await core.opAsync(
          "op_webusb_select_configuration",
          { rid: this[_handle], configurationValue },
        );
      }

      // 7.
      this.configuration = configuration;
    }

    async selectAlternateInterface(interfaceNumber, alternateSetting) {
      const prefix =
        "Failed to execute 'selectAlternateInterface' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      interfaceNumber = webidl.converters["octet"](interfaceNumber, {
        prefix,
        context: "Argument 1",
      });

      alternateSetting = webidl.converters["octet"](alternateSetting, {
        prefix,
        context: "Argument 2",
      });

      // 3.
      const _interface = this.configuration.interfaces.find((itf) =>
        itf.interfaceNumber == interfaceNumber
      );
      if (!_interface) {
        throw new DOMException(
          "Interface does not exist in active configuration.",
          "NotFoundError",
        );
      }

      // 4a.
      if (!this.opened) {
        throw new DOMException(
          "The device must be opened first.",
          "InvalidStateError",
        );
      }

      // 4b.
      if (!_interface.claimed) {
        throw new DOMException(
          "Interface must be claimed first.",
          "InvalidStateError",
        );
      }

      // 5. 6.
      if (!usbTest[_initialized]) {
        await core.opAsync(
          "op_webusb_select_alternate_interface",
          { rid: this[_handle], interfaceNumber, alternateSetting },
        );
      }
    }

    async clearHalt(direction, endpointNumber) {
      const prefix = "Failed to execute 'clearHalt' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      direction = webidl.converters["USBDirection"](direction, {
        prefix,
        context: "Argument 1",
      });

      endpointNumber = webidl.converters["octet"](endpointNumber, {
        prefix,
        context: "Argument 2",
      });

      const _interface = this.configuration.interfaces.find((itf) =>
        itf.alternates.find((alt) =>
          alt.endpoints.find((ep) => {
            return endpointNumber == ep.endpointNumber &&
              direction == ep.direction;
          })
        )
      );

      if (!_interface) {
        throw new DOMException(
          "Interface does not exist in active configuration.",
          "NotFoundError",
        );
      }

      // 3a.
      if (!this.opened) {
        throw new DOMException(
          "The device must be opened first.",
          "InvalidStateError",
        );
      }

      // 3b.
      if (!_interface.claimed) {
        throw new DOMException(
          "Interface must be claimed first.",
          "InvalidStateError",
        );
      }

      if (!usbTest[_initialized]) {
        // 4
        await core.opAsync(
          "op_webusb_clear_halt",
          { rid: this[_handle], direction, endpointNumber },
        );
      }
    }

    async controlTransferIn(setup, length) {
      const prefix = "Failed to execute 'controlTransferIn' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      setup = webidl.converters["USBControlTransferParameters"](setup, {
        prefix,
        context: "Argument 1",
      });

      length = webidl.converters["unsigned short"](data, {
        prefix,
        context: "Argument 2",
      });

      // 3.
      if (!this.opened) {
        throw new DOMException(
          "The device must be opened first.",
          "InvalidStateError",
        );
      }

      // 4.
      validateControlSetup(this.configuration, setup);

      if (!usbTest[_initialized]) {
        // 5 to 12.
        return await core.opAsync(
          "op_webusb_control_transfer_in",
          { rid: this[_handle], setup, length },
        );
      } else {
        return {
          status: "ok",
          data: new Uint8Array([
            length >> 8,
            length & 0xff,
            setup.request,
            setup.value >> 8,
            setup.value & 0xff,
            setup.index >> 8,
            setup.index & 0xff,
          ]),
        };
      }
    }

    async controlTransferOut(setup, data) {
      const prefix = "Failed to execute 'controlTransferOut' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      setup = webidl.converters["USBControlTransferParameters"](setup, {
        prefix,
        context: "Argument 1",
      });

      data = webidl.converters["BufferSource"](data, {
        prefix,
        context: "Argument 2",
      });

      // 2.
      if (!this.opened) {
        throw new DOMException(
          "The device must be opened first.",
          "InvalidStateError",
        );
      }

      // 3.
      validateControlSetup(this.configuration, setup);

      if (!usbTest[_initialized]) {
        // 4 to 9.
        return await core.opAsync(
          "op_webusb_control_transfer_out",
          { rid: this[_handle], setup },
          new Uint8Array(data),
        );
      } else {
        return {
          status: "ok",
          bytesWritten: data.byteLength,
        };
      }
    }

    async transferIn(endpointNumber, length) {
      const prefix = "Failed to execute 'transferIn' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      endpointNumber = webidl.converters["octet"](endpointNumber, {
        prefix,
        context: "Argument 1",
      });

      length = webidl.converters["unsigned long"](length, {
        prefix,
        context: "Argument 2",
      });

      // 3.
      const _interface = this.configuration.interfaces.find((itf) =>
        itf.alternates.find((alt) =>
          alt.endpoints.find((ep) => {
            return endpointNumber == ep.endpointNumber &&
              ep.direction == "in";
          })
        )
      );

      if (!_interface) {
        throw new DOMException(
          "Interface does not exist in active configuration.",
          "NotFoundError",
        );
      }

      // 5a.
      if (!this.opened) {
        throw new DOMException(
          "The device must be opened first.",
          "InvalidStateError",
        );
      }

      // 5b.
      if (!_interface.claimed) {
        throw new DOMException(
          "Interface must be claimed first.",
          "InvalidStateError",
        );
      }

      if (!usbTest[_initialized]) {
        // 6 to 15.
        return await core.opAsync(
          "op_webusb_transfer_in",
          { rid: this[_handle], endpointNumber, length },
        );
      } else {
        let data = new Array(length);
        for (let i = 0; i < length; ++i) {
          data[i] = i & 0xff;
        }
        return {
          status: "ok",
          data,
        };
      }
    }

    async transferOut(endpointNumber, data) {
      const prefix = "Failed to execute 'transferOut' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      endpointNumber = webidl.converters["octet"](endpointNumber, {
        prefix,
        context: "Argument 1",
      });

      data = webidl.converters["BufferSource"](data, {
        prefix,
        context: "Argument 2",
      });

      // 2.
      const _interface = this.configuration.interfaces.find((itf) =>
        itf.alternates.find((alt) =>
          alt.endpoints.find((ep) => {
            return endpointNumber == ep.endpointNumber &&
              ep.direction == "out";
          })
        )
      );

      if (!_interface) {
        throw new DOMException(
          "Interface does not exist in active configuration.",
          "NotFoundError",
        );
      }

      // 4a.
      if (!this.opened) {
        throw new DOMException(
          "The device must be opened first.",
          "InvalidStateError",
        );
      }

      // 4b.
      if (!_interface.claimed) {
        throw new DOMException(
          "Interface must be claimed first.",
          "InvalidStateError",
        );
      }

      if (!usbTest[_initialized]) {
        // 5 to 11.
        return await core.opAsync(
          "op_webusb_transfer_out",
          { rid: this[_handle], endpointNumber },
          new Uint8Array(data),
        );
      } else {
        return {
          status: "ok",
          bytesWritten: data.byteLength,
        };
      }
    }

    async reset() {
      webidl.assertBranded(this, UsbDevice);

      // 3.
      if (!this.opened) {
        throw new DOMException(
          "The device must be opened first.",
          "InvalidStateError",
        );
      }

      if (!usbTest[_initialized]) {
        // 4 to 6.
        await core.opAsync(
          "op_webusb_reset",
          this[_handle],
        );
      }
    }

    async open() {
      webidl.assertBranded(this, UsbDevice);

      // 3.
      if (!this.opened) {
        if (!usbTest[_initialized]) {
          const { rid } = await core.opAsync(
            "op_webusb_open_device",
            this[_rid],
          );

          // 5.
          this[_handle] = rid;
        }

        this.opened = true;
      }
    }

    async close() {
      webidl.assertBranded(this, UsbDevice);

      // 3.
      if (this.opened) {
        if (!usbTest[_initialized]) {
          await core.opAsync(
            "op_webusb_close_device",
            this[_handle],
          );
        }

        // 7.
        this.opened = false;
      }
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${inspect({})}`;
    }
  }

  mixinDevice(UsbDevice, _device);

  webidl.configurePrototype(UsbDevice);

  const _initialized = Symbol("initialized");
  const _devices = Symbol("devices");

  class USBTest {
    static #initialized = false;
    static #devices = [];

    constructor() {
      USBTest.#initialized = false;
    }

    get [_initialized]() {
      return USBTest.#initialized;
    }

    get [_devices]() {
      return USBTest.#devices;
    }

    async initialize() {
      USBTest.#initialized = true;
    }

    async addFakeDevice(deviceInit) {
      USBTest.#devices.push(deviceInit);
    }

    async reset() {
      USBTest.#devices = [];
      USBTest.#initialized = false;
    }
  }

  const usbTest = webidl.createBranded(USBTest);

  class USB {
    constructor() {
      webidl.illegalConstructor();
    }

    async getDevices() {
      let devices;
      if (!usbTest[_initialized]) {
        devices = await core.opAsync("op_webusb_get_devices", {});
      } else {
        devices = usbTest[_devices].map((usbdevice) => {
          return {
            rid: null,
            usbdevice,
          };
        });
      }

      return devices.map(({ rid, usbdevice }) => {
        let device = webidl.createBranded(UsbDevice);
        device[_rid] = rid;
        if (usbdevice.configuration) {
          usbdevice.configuration = new UsbConfiguration(
            usbdevice.configuration,
          );
        }

        usbdevice.configurations = usbdevice.configurations.map((config) =>
          new UsbConfiguration(config)
        );
        device[_device] = usbdevice;
        return device;
      });
    }

    async requestDevice({ filter }) {
      if (!usbTest[_initialized]) {
        // Request device. This adds it to the permission state.
        await core.opAsync("op_webusb_request_device", { ...filter });
      }
      // We re-use getDevices method here and filter through the allowed devices.
      const devices = await this.getDevices();
      return devices.filter((device) =>
        device.productId == filter.productId ||
        device.vendorId == filter.vendorId ||
        device.deviceProtocol == filter.protocolCode ||
        device.deviceSubclass == filter.subclassCode ||
        device.deviceClass == filter.classCode ||
        device.serialNumber == filter.serialNumber
      );
    }

    get test() {
      return usbTest;
    }
  }

  window.__bootstrap = window.__bootstrap || {};
  window.__bootstrap.usb = {
    usb: webidl.createBranded(USB),
    UsbDevice,
    UsbConfiguration,
  };
})(this);
