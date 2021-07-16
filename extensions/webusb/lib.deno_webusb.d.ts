// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

type USBDirection = "in" | "out";
type USBEndpointType = "bulk" | "interrupt" | "isochronous";
type USBRequestType = "standard" | "class" | "vendor";
type USBRecipient = "device" | "interface" | "endpoint" | "other";
type USBTransferStatus = "ok" | "stall" | "babble";

interface USBEndpoint {
  readonly endpointNumber: number;
  readonly direction: USBDirection;
  readonly type: USBEndpointType;
  readonly packetSize: number;
}

interface USBControlTransferParameters {
  requestType: USBRequestType;
  recipient: USBRecipient;
  request: number;
  value: number;
  index: number;
}

interface USBDeviceFilter {
  vendorId?: number;
  productId?: number;
  classCode?: number;
  subclassCode?: number;
  protocolCode?: number;
  serialNumber?: string;
}

interface USBDeviceRequestOptions {
  filters: USBDeviceFilter[];
}

interface USBConnectionEventInit extends EventInit {
  device: USBDevice;
}

declare class USBConfiguration {
  readonly configurationValue: number;
  readonly configurationName?: string;
  readonly interfaces: USBInterface[];
}

declare class USBInterface {
  constructor(configuration: USBConfiguration, interfaceNumber: number);
  readonly interfaceNumber: number;
  readonly alternate: USBAlternateInterface;
  readonly alternates: USBAlternateInterface[];
  readonly claimed: boolean;
}

declare class USBAlternateInterface {
  constructor(deviceInterface: USBInterface, alternateSetting: number);
  readonly alternateSetting: number;
  readonly interfaceClass: number;
  readonly interfaceSubclass: number;
  readonly interfaceProtocol: number;
  readonly interfaceName?: string;
  readonly endpoints: USBEndpoint[];
}

declare class USBInTransferResult {
  constructor(status: USBTransferStatus, data?: DataView);
  readonly data?: DataView;
  readonly status?: USBTransferStatus;
}

declare class USBOutTransferResult {
  constructor(status: USBTransferStatus, bytesWriten?: number);
  readonly bytesWritten: number;
  readonly status: USBTransferStatus;
}

declare class USBIsochronousInTransferPacket {
  constructor(status: USBTransferStatus, data?: DataView);
  readonly data?: DataView;
  readonly status?: USBTransferStatus;
}

declare class USBIsochronousInTransferResult {
  constructor(packets: USBIsochronousInTransferPacket[], data?: DataView);
  readonly data?: DataView;
  readonly packets: USBIsochronousInTransferPacket[];
}

declare class USBIsochronousOutTransferPacket {
  constructor(status: USBTransferStatus, bytesWritten?: number);
  readonly bytesWritten: number;
  readonly status: USBTransferStatus;
}

declare class USBConnectionEvent extends Event {
  constructor(type: string, eventInitDict: USBConnectionEventInit);
  readonly device: USBDevice;
}

declare class USBIsochronousOutTransferResult {
  constructor(packets: USBIsochronousOutTransferPacket[]);
  readonly packets: USBIsochronousOutTransferPacket[];
}

declare class USB extends EventTarget {
  getDevices(): Promise<USBDevice[]>;
  requestDevice(options?: USBDeviceRequestOptions): Promise<USBDevice>;
}

declare class USBDevice {
  readonly usbVersionMajor: number;
  readonly usbVersionMinor: number;
  readonly usbVersionSubminor: number;
  readonly deviceClass: number;
  readonly deviceSubclass: number;
  readonly deviceProtocol: number;
  readonly vendorId: number;
  readonly productId: number;
  readonly deviceVersionMajor: number;
  readonly deviceVersionMinor: number;
  readonly deviceVersionSubminor: number;
  readonly manufacturerName?: string;
  readonly productName?: string;
  readonly serialNumber?: string;
  readonly configuration?: USBConfiguration;
  readonly configurations: USBConfiguration[];
  readonly opened: boolean;
  open(): Promise<void>;
  close(): Promise<void>;
  selectConfiguration(configurationValue: number): Promise<void>;
  claimInterface(interfaceNumber: number): Promise<void>;
  releaseInterface(interfaceNumber: number): Promise<void>;
  selectAlternateInterface(
    interfaceNumber: number,
    alternateSetting: number,
  ): Promise<void>;
  controlTransferIn(
    setup: USBControlTransferParameters,
    length: number,
  ): Promise<USBInTransferResult>;
  controlTransferOut(
    setup: USBControlTransferParameters,
    data?: Uint8Array,
  ): Promise<USBOutTransferResult>;
  clearHalt(direction: USBDirection, endpointNumber: number): Promise<void>;
  transferIn(
    endpointNumber: number,
    length: number,
  ): Promise<USBInTransferResult>;
  transferOut(
    endpointNumber: number,
    data: Uint8Array,
  ): Promise<USBOutTransferResult>;
  isochronousTransferIn(
    endpointNumber: number,
    packetLengths: number[],
  ): Promise<USBIsochronousInTransferResult>;
  isochronousTransferOut(
    endpointNumber: number,
    data: Uint8Array,
    packetLengths: number[],
  ): Promise<USBIsochronousOutTransferResult>;
  reset(): Promise<void>;
}

interface Navigator {
  readonly usb: USB;
}
