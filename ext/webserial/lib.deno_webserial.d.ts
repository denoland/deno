// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare class Serial {
  getPorts(): Promise<SerialPort[]>;
  requestPort(options?: SerialPortRequestOptions): Promise<SerialPort>;
}

declare class SerialPort {
  readonly readable: ReadableStream<Uint8Array> | null;
  readonly writable: WritableStream<Uint8Array> | null;

  getInfo(): SerialPortInfo;

  open(options: SerialOptions): Promise<void>;
  setSignals(signals?: SerialOutputSignals): Promise<void>;
  getSignals(): Promise<SerialInputSignals>;
  close(): Promise<void>;
}

declare interface SerialPortRequestOptions {
  filters?: SerialPortFilter[];
}

declare interface SerialPortFilter {
  usbVendorId?: number;
  usbProductId?: number;
}

declare interface SerialPortInfo {
  usbVendorId?: number;
  usbProductId?: number;
}

declare interface SerialOptions {
  baudRate: number;
  dataBits?: number;
  stopBits?: number;
  parity?: ParityType;
  bufferSize?: number;
  flowControl?: FlowControlType;
}

declare type ParityType = "none" | "even" | "odd";

declare type FlowControlType = "none" | "hardware";

declare interface SerialOutputSignals {
  dataTerminalReady?: boolean;
  requestToSend?: boolean;
  break?: boolean;
}

declare interface SerialInputSignals {
  dataCarrierDetect: boolean;
  clearToSend: boolean;
  ringIndicator: boolean;
  dataSetReady: boolean;
}
