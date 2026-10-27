// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertArrayEquals, test } from "checkin:testing";
const {
  op_v8slice_store,
  op_v8slice_clone,
  op_v8slice_into_bytes,
  op_v8slice_into_bytes_aliases,
} = Deno.core.ops;

// Cloning a buffer should result in the same buffer being returned
test(function testBufferStore() {
  const data = new Uint8Array(1024 * 1024);
  op_v8slice_store("buffer", data);
  const output = op_v8slice_clone("buffer");
  assertArrayEquals(output, new Uint8Array(1024 * 1024));
});

// Ensure that the returned buffer size is correct when a buffer is resized
// externally via `ArrayBuffer.transfer`.
test(function testBufferTransfer() {
  const data = new Uint8Array(1024 * 1024);
  const buffer = data.buffer;
  op_v8slice_store("buffer", data);
  buffer.transfer(100);
  const output = op_v8slice_clone("buffer");
  // Note: after https://chromium-review.googlesource.com/c/v8/v8/+/5394731 landed, the underlying
  // AB backingstore is no longer resized.
  assertArrayEquals(output, new Uint8Array(1024 * 1024));
});

// Exercises the `JsBuffer` arm of `BufView::into_bytes` (the zero-copy handoff
// used by the fetch/http upload paths). A fixed-length ArrayBuffer takes the
// zero-copy path.
test(function testBufferIntoBytes() {
  const data = new Uint8Array([1, 2, 3, 4, 5]);
  const output = op_v8slice_into_bytes(data);
  assertArrayEquals(output, data);

  // A view with a non-zero byte offset must expose only the viewed sub-range.
  const backing = new Uint8Array([10, 20, 30, 40, 50, 60]);
  const view = backing.subarray(2, 5);
  assertArrayEquals(op_v8slice_into_bytes(view), new Uint8Array([30, 40, 50]));
});

// Resizable/shared stores can't take the zero-copy path (they may shrink or be
// mutated concurrently while the bytes are still in flight), so `into_bytes`
// copies. The contents must still round-trip correctly.
test(function testBufferIntoBytesResizable() {
  const ab = new ArrayBuffer(4, { maxByteLength: 8 });
  const data = new Uint8Array(ab);
  data.set([7, 8, 9, 10]);
  assertArrayEquals(op_v8slice_into_bytes(data), new Uint8Array([7, 8, 9, 10]));
});

// Pins the safety guard directly: `into_bytes` must hand off fixed-length
// stores zero-copy (aliasing the source), and must copy resizable/shared
// stores (not aliasing). Deleting the `is_backing_store_resizable` /
// `is_backing_store_shared` check flips these and fails the test.
test(function testBufferIntoBytesAliasing() {
  // Fixed-length: zero-copy handoff, aliases the source store.
  const fixed = new Uint8Array([1, 2, 3, 4]);
  assert(op_v8slice_into_bytes_aliases(fixed), "fixed-length must alias");

  // Resizable: copied, must not alias.
  const rab = new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 }));
  assert(!op_v8slice_into_bytes_aliases(rab), "resizable must copy");

  // Shared: copied, must not alias.
  const sab = new Uint8Array(new SharedArrayBuffer(4));
  assert(!op_v8slice_into_bytes_aliases(sab), "shared must copy");
});
