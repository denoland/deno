// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

type RawBaseType = "int8" | "int16" | "int32" | "uint8" | "uint16" | "uint32";
type RawNumberType = RawBaseType | "float32" | "float64";
type RawBigType = RawBaseType | "int64" | "uint64";
export type DataType = RawNumberType | RawBigType;

/** How encoded binary data is ordered. */
export type Endianness = "little" | "big";

/** Options for working with the `number` type. */
export interface VarnumOptions {
  /** The binary format used. */
  dataType?: RawNumberType;
  /** The binary encoding order used. */
  endian?: Endianness;
}

/** Options for working with the `bigint` type. */
export interface VarbigOptions {
  /** The binary format used. */
  dataType?: RawBigType;
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
 * Resolves it in a `Uint8Array`, or throws `Deno.errors.UnexpectedEof` if `n` bytes cannot be read. */
export async function getNBytes(
  r: Deno.Reader,
  n: number,
): Promise<Uint8Array> {
  const scratch = new Uint8Array(n);
  const nRead = await r.read(scratch);
  if (nRead === null || nRead < n) throw new Deno.errors.UnexpectedEof();
  return scratch;
}

/** Decodes a number from `b`. If `o.bytes` is shorter than `sizeof(o.dataType)`, returns `null`.
 *
 * `o.dataType` defaults to `"int32"`. */
export function varnum(b: Uint8Array, o: VarnumOptions = {}): number | null {
  o.dataType = o.dataType ?? "int32";
  const littleEndian = (o.endian ?? "big") === "little" ? true : false;
  if (b.length < sizeof(o.dataType)) return null;
  const view = new DataView(b.buffer);
  switch (o.dataType) {
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

/** Decodes a bigint from `b`. If `o.bytes` is shorter than `sizeof(o.dataType)`, returns `null`.
 *
 * `o.dataType` defaults to `"int64"`. */
export function varbig(b: Uint8Array, o: VarbigOptions = {}): bigint | null {
  o.dataType = o.dataType ?? "int64";
  const littleEndian = (o.endian ?? "big") === "little" ? true : false;
  if (b.length < sizeof(o.dataType)) return null;
  const view = new DataView(b.buffer);
  switch (o.dataType) {
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

/** Encodes number `x` into `b`. Returns the number of bytes used, or `0` if `b` is shorter than `sizeof(o.dataType)`.
 *
 * `o.dataType` defaults to `"int32"`. */
export function putVarnum(
  b: Uint8Array,
  x: number,
  o: VarnumOptions = {},
): number {
  o.dataType = o.dataType ?? "int32";
  const littleEndian = (o.endian ?? "big") === "little" ? true : false;
  if (b.length < sizeof(o.dataType)) return 0;
  const view = new DataView(b.buffer);
  switch (o.dataType) {
    case "int8":
      view.setInt8(0, x);
      break;
    case "uint8":
      view.setUint8(0, x);
      break;
    case "int16":
      view.setInt16(0, x, littleEndian);
      break;
    case "uint16":
      view.setUint16(0, x, littleEndian);
      break;
    case "int32":
      view.setInt32(0, x, littleEndian);
      break;
    case "uint32":
      view.setUint32(0, x, littleEndian);
      break;
    case "float32":
      view.setFloat32(0, x, littleEndian);
      break;
    case "float64":
      view.setFloat64(0, x, littleEndian);
      break;
  }
  return sizeof(o.dataType);
}

/** Encodes bigint `x` into `b`. Returns the number of bytes used, or `0` if `b` is shorter than `sizeof(o.dataType)`.
 *
 * `o.dataType` defaults to `"int64"`. */
export function putVarbig(
  b: Uint8Array,
  x: bigint,
  o: VarbigOptions = {},
): number {
  o.dataType = o.dataType ?? "int64";
  const littleEndian = (o.endian ?? "big") === "little" ? true : false;
  if (b.length < sizeof(o.dataType)) return 0;
  const view = new DataView(b.buffer);
  switch (o.dataType) {
    case "int8":
      view.setInt8(0, Number(x));
      break;
    case "uint8":
      view.setUint8(0, Number(x));
      break;
    case "int16":
      view.setInt16(0, Number(x), littleEndian);
      break;
    case "uint16":
      view.setUint16(0, Number(x), littleEndian);
      break;
    case "int32":
      view.setInt32(0, Number(x), littleEndian);
      break;
    case "uint32":
      view.setUint32(0, Number(x), littleEndian);
      break;
    case "int64":
      view.setBigInt64(0, x, littleEndian);
      break;
    case "uint64":
      view.setBigUint64(0, x, littleEndian);
      break;
  }
  return sizeof(o.dataType);
}

/** Decodes a number from `r`, comsuming `sizeof(o.dataType)` bytes. If less than `sizeof(o.dataType)` bytes were read, throws `Deno.errors.unexpectedEof`.
 *
 * `o.dataType` defaults to `"int32"`. */
export async function readVarnum(
  r: Deno.Reader,
  o: VarnumOptions = {},
): Promise<number> {
  o.dataType = o.dataType ?? "int32";
  const scratch = await getNBytes(r, sizeof(o.dataType));
  return varnum(scratch, o) as number;
}

/** Decodes a bigint from `r`, comsuming `sizeof(o.dataType)` bytes. If less than `sizeof(o.dataType)` bytes were read, throws `Deno.errors.unexpectedEof`.
 *
 * `o.dataType` defaults to `"int64"`. */
export async function readVarbig(
  r: Deno.Reader,
  o: VarbigOptions = {},
): Promise<bigint> {
  o.dataType = o.dataType ?? "int64";
  const scratch = await getNBytes(r, sizeof(o.dataType));
  return varbig(scratch, o) as bigint;
}

/** Encodes and writes `x` to `w`. Resolves to the number of bytes written.
 *
 * `o.dataType` defaults to `"int32"`. */
export function writeVarnum(
  w: Deno.Writer,
  x: number,
  o: VarnumOptions = {},
): Promise<number> {
  o.dataType = o.dataType ?? "int32";
  const scratch = new Uint8Array(sizeof(o.dataType));
  putVarnum(scratch, x, o);
  return w.write(scratch);
}

/** Encodes and writes `x` to `w`. Resolves to the number of bytes written.
 *
 * `o.dataType` defaults to `"int64"`. */
export function writeVarbig(
  w: Deno.Writer,
  x: bigint,
  o: VarbigOptions = {},
): Promise<number> {
  o.dataType = o.dataType ?? "int64";
  const scratch = new Uint8Array(sizeof(o.dataType));
  putVarbig(scratch, x, o);
  return w.write(scratch);
}

/** Encodes `x` into a new `Uint8Array`.
 *
 * `o.dataType` defaults to `"int32"` */
export function varnumBytes(x: number, o: VarnumOptions = {}): Uint8Array {
  o.dataType = o.dataType ?? "int32";
  const b = new Uint8Array(sizeof(o.dataType));
  putVarnum(b, x, o);
  return b;
}

/** Encodes `x` into a new `Uint8Array`.
 *
 * `o.dataType` defaults to `"int64"` */
export function varbigBytes(x: bigint, o: VarbigOptions = {}): Uint8Array {
  o.dataType = o.dataType ?? "int64";
  const b = new Uint8Array(sizeof(o.dataType));
  putVarbig(b, x, o);
  return b;
}
