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
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      return await core.opAsync(
        "op_webusb_claim_interface",
        { rid: this.#deviceHandleRid, interfaceNumber },
      );
    }

    async releaseInterface(interfaceNumber) {
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      return await core.opAsync(
        "op_webusb_release_interface",
        { rid: this.#deviceHandleRid, interfaceNumber },
      );
    }

    async selectConfiguration(configurationValue) {
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      return await core.opAsync(
        "op_webusb_select_configuration",
        { rid: this.#deviceHandleRid, configurationValue },
      );
    }

    async selectAlternateInterface(interfaceNumber, alternateSetting) {
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      return await core.opAsync(
        "op_webusb_select_alternate_interface",
        { rid: this.#deviceHandleRid, interfaceNumber, alternateSetting },
      );
    }

    async clearHalt(direction, endpointNumber) {
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      return await core.opAsync(
        "op_webusb_clear_halt",
        { rid: this.#deviceHandleRid, direction, endpointNumber },
      );
    }

    async controlTransferOut(setup, data) {
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      return await core.opAsync(
        "op_webusb_control_transfer_out",
        { rid: this.#deviceHandleRid, setup },
        new Uint8Array(data),
      );
    }

    async controlTransferIn(setup, length) {
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      return await core.opAsync(
        "op_webusb_control_transfer_in",
        { rid: this.#deviceHandleRid, setup, length },
      );
    }

    async transferIn(endpointNumber, length) {
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      return await core.opAsync(
        "op_webusb_transfer_in",
        { rid: this.#deviceHandleRid, endpointNumber, length },
      );
    }

    async transferOut(endpointNumber, data) {
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      return await core.opAsync(
        "op_webusb_transfer_out",
        { rid: this.#deviceHandleRid, endpointNumber },
        new Uint8Array(data),
      );
    }

    async reset() {
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      return await core.opAsync(
        "op_webusb_reset",
        { rid: this.#deviceHandleRid },
      );
    }

    async open() {
      webidl.assertBranded(this, UsbDevice);

      if (this.opened) throw new Error("The device is already opened.");
      const { rid } = await core.opAsync(
        "op_webusb_open_device",
        { rid: this.#rid },
      );
      this.#deviceHandleRid = rid;
      this.opened = true;
    }

    async close() {
      webidl.assertBranded(this, UsbDevice);

      if (!this.opened) throw new Error("The device must be opened first.");
      await core.opAsync("op_webusb_close_device", {
        rid: this.#deviceHandleRid,
      });
      this.opened = false;
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
