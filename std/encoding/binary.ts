// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

type RawBaseType = "int8" | "int16" | "int32" | "uint8" | "uint16" | "uint32";
type RawNumberType = RawBaseType | "float32" | "float64";
type RawBigType = RawBaseType | "int64" | "uint64";
export type DataType = RawNumberType | RawBigType;

/** How encoded binary data is ordered. */
export type Endianness = "little" | "big";

/** Options for working with the `number` type. */
export interface VarnumOptions {
  /** The binary format used. */
  type: RawNumberType;
  /** The binary encoding order used. */
  endian?: Endianness;
}

/** Options for working with the `bigint` type. */
export interface VarbigOptions {
  /** The binary format used. */
  type: RawBigType;
  /** The binary encoding order used. */
  endian?: Endianness;
}

const rawTypeSizes: Record<DataType, number> = {
  int8: 1,
  uint8: 1,
  int16: 2,
  uint16: 2,
  int32: 4,
  uint32: 4,
  int64: 8,
  uint64: 8,
  float32: 4,
  float64: 8,
} as const;

/** Number of bytes required to store `dataType`. */
export function sizeof(dataType: DataType): number {
  return rawTypeSizes[dataType];
}

/** Reads `n` bytes from `r`.
 *
 * Returns it in a `Uint8Array`, or null if `n` bytes cannot be read. */
export async function getNBytes(
  r: Deno.Reader,
  n: number
): Promise<Uint8Array | null> {
  const scratch = new Uint8Array(n);
  const nRead = await r.read(scratch);
  if (nRead === null || nRead < n) return null;
  return scratch;
}

/** Decode a `number` from `o.bytes`.
 *
 * If `o.bytes` is shorter than `sizeof(o.type)`, returns `null`. */
export function varnum(
  o: VarnumOptions & { bytes: Uint8Array }
): number | null {
  const littleEndian = (o.endian ?? "big") === "little" ? true : false;
  if (o.bytes.length < sizeof(o.type)) return null;
  const view = new DataView(o.bytes.buffer);
  switch (o.type) {
    case "int8":
      return view.getInt8(0);
    case "uint8":
      return view.getUint8(0);
    case "int16":
      return view.getInt16(0, littleEndian);
    case "uint16":
      return view.getUint16(0, littleEndian);
    case "int32":
      return view.getInt32(0, littleEndian);
    case "uint32":
      return view.getUint32(0, littleEndian);
    case "float32":
      return view.getFloat32(0, littleEndian);
    case "float64":
      return view.getFloat64(0, littleEndian);
  }
}

/** Decode a `bigint` from `o.bytes`.
 *
 * If `o.bytes` is shorter than `sizeof(o.type)`, returns `null`. */
export function varbig(
  o: VarbigOptions & { bytes: Uint8Array }
): bigint | null {
  const littleEndian = (o.endian ?? "big") === "little" ? true : false;
  if (o.bytes.length < sizeof(o.type)) return null;
  const view = new DataView(o.bytes.buffer);
  switch (o.type) {
    case "int8":
      return BigInt(view.getInt8(0));
    case "uint8":
      return BigInt(view.getUint8(0));
    case "int16":
      return BigInt(view.getInt16(0, littleEndian));
    case "uint16":
      return BigInt(view.getUint16(0, littleEndian));
    case "int32":
      return BigInt(view.getInt32(0, littleEndian));
    case "uint32":
      return BigInt(view.getUint32(0, littleEndian));
    case "int64":
      return view.getBigInt64(0, littleEndian);
    case "uint64":
      return view.getBigUint64(0, littleEndian);
  }
}

/** Encode `o.value` into `o.bytes`.
 *
 * Returns the number of bytes used, or `0` if `o.bytes` is shorter than `sizeof(o.type)`. */
export function putVarnum(
  o: VarnumOptions & { bytes: Uint8Array; value: number }
): number {
  const littleEndian = (o.endian ?? "big") === "little" ? true : false;
  if (o.bytes.length < sizeof(o.type)) return 0;
  const view = new DataView(o.bytes.buffer);
  switch (o.type) {
    case "int8":
      view.setInt8(0, o.value);
      break;
    case "uint8":
      view.setUint8(0, o.value);
      break;
    case "int16":
      view.setInt16(0, o.value, littleEndian);
      break;
    case "uint16":
      view.setUint16(0, o.value, littleEndian);
      break;
    case "int32":
      view.setInt32(0, o.value, littleEndian);
      break;
    case "uint32":
      view.setUint32(0, o.value, littleEndian);
      break;
    case "float32":
      view.setFloat32(0, o.value, littleEndian);
      break;
    case "float64":
      view.setFloat64(0, o.value, littleEndian);
      break;
  }
  return sizeof(o.type);
}

/** Encode `o.value` into `o.bytes`.
 *
 * Returns the number of bytes used, or `0` if `o.bytes` is shorter than `sizeof(o.type)`. */
export function putVarbig(
  o: VarbigOptions & { bytes: Uint8Array; value: bigint }
): number {
  const littleEndian = (o.endian ?? "big") === "little" ? true : false;
  if (o.bytes.length < sizeof(o.type)) return 0;
  const view = new DataView(o.bytes.buffer);
  switch (o.type) {
    case "int8":
      view.setInt8(0, Number(o.value));
      break;
    case "uint8":
      view.setUint8(0, Number(o.value));
      break;
    case "int16":
      view.setInt16(0, Number(o.value), littleEndian);
      break;
    case "uint16":
      view.setUint16(0, Number(o.value), littleEndian);
      break;
    case "int32":
      view.setInt32(0, Number(o.value), littleEndian);
      break;
    case "uint32":
      view.setUint32(0, Number(o.value), littleEndian);
      break;
    case "int64":
      view.setBigInt64(0, o.value, littleEndian);
      break;
    case "uint64":
      view.setBigUint64(0, o.value, littleEndian);
      break;
  }
  return sizeof(o.type);
}

/** Decodes a number from `r`, comsuming `sizeof(o.type)` bytes.
 *
 * If less than `sizeof(o.type)` bytes were read, returns `null`. */
export async function readVarnum(
  o: VarnumOptions & { src: Deno.Reader }
): Promise<number | null> {
  const scratch = await getNBytes(o.src, sizeof(o.type));
  if (scratch === null) return null;
  return varnum({ bytes: scratch, type: o.type, endian: o.endian }) as number;
}

/** Decodes a bigint from `r`, comsuming `sizeof(o.type)` bytes.
 *
 * If less than `sizeof(o.type)` bytes were read, returns `null`. */
export async function readVarbig(
  o: VarbigOptions & { src: Deno.Reader }
): Promise<bigint | null> {
  const scratch = await getNBytes(o.src, sizeof(o.type));
  if (scratch === null) return null;
  return varbig({ bytes: scratch, type: o.type, endian: o.endian }) as bigint;
}

/** Encodes and writes `o.value` to `o.dst`.
 *
 * Returns the number of bytes written. */
export function writeVarnum(
  o: VarnumOptions & { dst: Deno.Writer; value: number }
): Promise<number> {
  const scratch = new Uint8Array(sizeof(o.type));
  const nWritten = putVarnum({
    type: o.type,
    bytes: scratch,
    value: o.value,
    endian: o.endian,
  });
  return o.dst.write(scratch.subarray(0, nWritten));
}

/** Encodes and writes `o.value` to `o.dst`.
 *
 * Returns the number of bytes written. */
export function writeVarbig(
  o: VarbigOptions & { dst: Deno.Writer; value: bigint }
): Promise<number> {
  const scratch = new Uint8Array(sizeof(o.type));
  const nWritten = putVarbig({
    type: o.type,
    bytes: scratch,
    value: o.value,
    endian: o.endian,
  });
  return o.dst.write(scratch.subarray(0, nWritten));
}

/** Encodes `o.value` into a new `Uint8Array`. */
export function varnumBytes(o: VarnumOptions & { value: number }): Uint8Array {
  const b = new Uint8Array(sizeof(o.type));
  putVarnum({ bytes: b, type: o.type, value: o.value, endian: o.endian });
  return b;
}

/** Encodes `o.value` into a new `Uint8Array`. */
export function varbigBytes(o: VarbigOptions & { value: bigint }): Uint8Array {
  const b = new Uint8Array(sizeof(o.type));
  putVarbig({ bytes: b, type: o.type, value: o.value, endian: o.endian });
  return b;
}
