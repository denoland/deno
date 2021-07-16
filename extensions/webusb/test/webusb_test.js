import {
  assert,
  assertEquals,
} from "https://deno.land/std@0.99.0/testing/asserts.ts";

let devices;
let g_zero;

Deno.test({
  name: "enumerate devices",
  fn: async () => {
    devices = await navigator.usb.getDevices();
    assert(devices);
    assert(devices.length >= 1, "No devices found");
  },
  sanitizeResources: false,
});

Deno.test({
  name: "find test device",
  fn: async () => {
    assert(devices);
    g_zero = devices.find((dev) =>
      dev.vendorId == 0x0525 && dev.productId == 0xa4a0
    );
    assert(g_zero instanceof USBDevice);
    assert(g_zero, "Test device not found");
  },
  sanitizeResources: false,
});

Deno.test({
  name: "open device",
  fn: async () => {
    assert(g_zero instanceof USBDevice);
    await g_zero.open();
    assertEquals(g_zero.opened, true);
  },
  sanitizeResources: false,
});

Deno.test({
  name: "claim interface #0",
  fn: async () => {
    assert(g_zero instanceof USBDevice);
    assert(g_zero.opened);
    await g_zero.claimInterface(0);
    const itf = g_zero.configuration.interfaces.find((itf) =>
      itf.interfaceNumber == 0
    );
    assert(itf);
    assertEquals(itf.claimed, true);
  },
  sanitizeResources: false,
});

// deno-fmt-ignore
const IN_DATA =  [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 0, 1, 2, 3, 4, 5, 6, 7];
const OUT_DATA = new Uint8Array([0, 0, 0, 0, 0, 0, 0, 0]);

Deno.test({
  name: "transfer in #1",
  fn: async () => {
    assert(g_zero instanceof USBDevice);
    assert(g_zero.opened);
    const transferInResult = await g_zero.transferIn(1, 512);
    assertEquals(transferInResult.status, "completed");
    assertEquals(transferInResult.data, IN_DATA);
  },
  sanitizeResources: false,
});

Deno.test({
  name: "transfer out #2",
  fn: async () => {
    assert(g_zero instanceof USBDevice);
    assert(g_zero.opened);
    const transferOutResult = await g_zero.transferOut(2, OUT_DATA);
    assertEquals(transferOutResult.status, "completed");
    assertEquals(transferOutResult.bytesWritten, OUT_DATA.byteLength);
  },
  sanitizeResources: false,
});

Deno.test({
  name: "control transfer out #11",
  fn: async () => {
    assert(g_zero instanceof USBDevice);
    assert(g_zero.opened);
    const controlTransferOutResult = await g_zero.controlTransferOut({
      requestType: "standard",
      recipient: "interface",
      request: 11,
      value: 0,
      index: 0,
    }, new Uint8Array());

    assertEquals(controlTransferOutResult.status, "completed");
    assertEquals(controlTransferOutResult.bytesWritten, 0);
  },
  sanitizeResources: false,
});

Deno.test({
  name: "close test device",
  fn: async () => {
    assert(g_zero instanceof USBDevice);
    assert(g_zero.opened);
    await g_zero.close();
    assert(!g_zero.opened);
  },
  sanitizeResources: false,
});
