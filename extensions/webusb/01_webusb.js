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

  class UsbDevice {
    #rid;
    #deviceHandleRid;
    constructor(device, rid) {
      Object.assign(this, device);
      this.configurations = device.configurations.map((config) =>
        new UsbConfiguration(config)
      );
      this.#rid = rid;
      this.opened = false;
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
        // 5.
        await core.opAsync(
          "op_webusb_claim_interface",
          { rid: this.#deviceHandleRid, interfaceNumber },
        );

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
        await core.opAsync(
          "op_webusb_release_interface",
          { rid: this.#deviceHandleRid, interfaceNumber },
        );

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

      await core.opAsync(
        "op_webusb_select_configuration",
        { rid: this.#deviceHandleRid, configurationValue },
      );

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
      await core.opAsync(
        "op_webusb_select_alternate_interface",
        { rid: this.#deviceHandleRid, interfaceNumber, alternateSetting },
      );
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

      // 4
      await core.opAsync(
        "op_webusb_clear_halt",
        { rid: this.#deviceHandleRid, direction, endpointNumber },
      );
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
      if (this.configuration) {
        // https://wicg.github.io/webusb/#check-the-validity-of-the-control-transfer-parameters

        // 4.
        if (setup.recipient == "interface") {
          // 1.
          const interfaceNumber = setup.index & 0xFF;
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

          // 4.
          if (!_interface.claimed) {
            throw new DOMException(
              "Interface must be claimed first.",
              "InvalidStateError",
            );
          }
        }
      }

      // 5 to 12.
      return await core.opAsync(
        "op_webusb_control_transfer_in",
        { rid: this.#deviceHandleRid, setup, length },
      );
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
      // https://wicg.github.io/webusb/#check-the-validity-of-the-control-transfer-parameters
      if (this.configuration) {
        // 4.
        if (setup.recipient == "interface") {
          // 1.
          const interfaceNumber = setup.index & 0xFF;
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

          // 4.
          if (!_interface.claimed) {
            throw new DOMException(
              "Interface must be claimed first.",
              "InvalidStateError",
            );
          }
        }
      }

      // 4 to 9.
      return await core.opAsync(
        "op_webusb_control_transfer_out",
        { rid: this.#deviceHandleRid, setup },
        new Uint8Array(data),
      );
    }

    async transferIn(endpointNumber, length) {
      const prefix = "Failed to execute 'transferIn' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      endpointNumber = webidl.converters["octet"](interfaceNumber, {
        prefix,
        context: "Argument 1",
      });

      length = webidl.converters["unsigned long"](alternateSetting, {
        prefix,
        context: "Argument 2",
      });

      // 3.
      const _interface = this.configuration.interfaces.find((itf) =>
        itf.alternates.find((alt) =>
          alt.endpoints.find((ep) => {
            return endpointNumber == ep.endpointNumber &&
              direction == "in";
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

      // 6 to 15.
      return await core.opAsync(
        "op_webusb_transfer_in",
        { rid: this.#deviceHandleRid, endpointNumber, length },
      );
    }

    async transferOut(endpointNumber, data) {
      const prefix = "Failed to execute 'transferOut' on 'USBDevice'";

      webidl.assertBranded(this, UsbDevice);

      endpointNumber = webidl.converters["octet"](interfaceNumber, {
        prefix,
        context: "Argument 1",
      });

      data = webidl.converters["BufferSource"](alternateSetting, {
        prefix,
        context: "Argument 2",
      });

      // 2.
      const _interface = this.configuration.interfaces.find((itf) =>
        itf.alternates.find((alt) =>
          alt.endpoints.find((ep) => {
            return endpointNumber == ep.endpointNumber &&
              direction == "out";
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

      // 5 to 11.
      return await core.opAsync(
        "op_webusb_transfer_out",
        { rid: this.#deviceHandleRid, endpointNumber },
        new Uint8Array(data),
      );
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

      // 4 to 6.
      await core.opAsync(
        "op_webusb_reset",
        { rid: this.#deviceHandleRid },
      );
    }

    async open() {
      webidl.assertBranded(this, UsbDevice);

      // 3.
      if (!this.opened) {
        const { rid } = await core.opAsync(
          "op_webusb_open_device",
          { rid: this.#rid },
        );

        // 5.
        this.#deviceHandleRid = rid;
        this.opened = true;
      }
    }

    async close() {
      webidl.assertBranded(this, UsbDevice);

      // 3.
      if (this.opened) {
        await core.opAsync("op_webusb_close_device", {
          rid: this.#deviceHandleRid,
        });

        // 7.
        this.opened = false;
      }
    }
  }

  async function getDevices() {
    const devices = await core.opAsync("op_webusb_get_devices", {});
    return devices.map(({ rid, usbdevice }) => new UsbDevice(usbdevice, rid));
  }

  async function requestDevice({ filter }) {
    // Request device. This adds it to the permission state.
    await core.opAsync("op_webusb_request_device", { ...filter });
    // We re-use getDevices method here and filter through the allowed devices.
    const devices = await getDevices();
    return devices.filter((device) =>
      device.productId == filter.productId ||
      device.vendorId == filter.vendorId ||
      device.deviceProtocol == filter.protocolCode ||
      device.deviceSubclass == filter.subclassCode ||
      device.deviceClass == filter.classCode ||
      device.serialNumber == filter.serialNumber
    );
  }

  window.usb = {
    requestDevice,
    getDevices,
    UsbDevice,
    UsbConfiguration,
  };
  window.__bootstrap = window.__bootstrap || {};
  window.__bootstrap.usb = {
    requestDevice,
    getDevices,
    UsbDevice,
    UsbConfiguration,
  };
})(this);
