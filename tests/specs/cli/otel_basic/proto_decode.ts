// Copyright 2018-2026 the Deno authors. MIT license.

// Minimal protobuf decoder for OTLP messages. Uses the JSON proto descriptor
// (otlp_proto.json) as the schema. No external dependencies.

const descriptor = JSON.parse(
  Deno.readTextFileSync(new URL("./otlp_proto.json", import.meta.url)),
);

// Build a lookup table: fully-qualified type name -> type definition
const typeRegistry = new Map();

function registerTypes(node, prefix) {
  if (!node.nested) return;
  for (const [name, def] of Object.entries(node.nested)) {
    const fqn = prefix ? `${prefix}.${name}` : name;
    if (def.fields) {
      typeRegistry.set(fqn, def);
    }
    if (def.nested) {
      registerTypes(def, fqn);
    }
  }
}
registerTypes(descriptor, "");

// Wire types
const VARINT = 0;
const I64 = 1;
const LEN = 2;
const I32 = 5;

class Reader {
  buf;
  pos;
  end;

  constructor(buf, start, end) {
    this.buf = buf;
    this.pos = start || 0;
    this.end = end !== undefined ? end : buf.length;
  }

  varint() {
    let result = 0n;
    let shift = 0n;
    while (this.pos < this.end) {
      const b = this.buf[this.pos++];
      result |= BigInt(b & 0x7f) << shift;
      if ((b & 0x80) === 0) return result;
      shift += 7n;
    }
    throw new Error("varint overflow");
  }

  fixed32() {
    const v = (this.buf[this.pos]) |
      (this.buf[this.pos + 1] << 8) |
      (this.buf[this.pos + 2] << 16) |
      (this.buf[this.pos + 3] << 24);
    this.pos += 4;
    return v >>> 0;
  }

  fixed64() {
    const lo = BigInt(this.fixed32());
    const hi = BigInt(this.fixed32());
    return (hi << 32n) | lo;
  }

  float64() {
    const dv = new DataView(
      this.buf.buffer,
      this.buf.byteOffset + this.pos,
      8,
    );
    this.pos += 8;
    return dv.getFloat64(0, true);
  }

  float32() {
    const dv = new DataView(
      this.buf.buffer,
      this.buf.byteOffset + this.pos,
      4,
    );
    this.pos += 4;
    return dv.getFloat32(0, true);
  }

  skip(wireType) {
    switch (wireType) {
      case VARINT:
        this.varint();
        break;
      case I64:
        this.pos += 8;
        break;
      case LEN: {
        const len = Number(this.varint());
        this.pos += len;
        break;
      }
      case I32:
        this.pos += 4;
        break;
      // Groups (wire types 3 and 4) are deprecated but should be skippable
      case 3: // SGROUP - skip until matching EGROUP
      case 4: // EGROUP
        break;
      default:
        throw new Error(`unknown wire type ${wireType}`);
    }
  }
}

function bytesToHex(bytes) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

// Resolve a protobuf type reference relative to a context type name.
function resolveType(typeName, context) {
  if (typeRegistry.has(typeName)) return typeRegistry.get(typeName);
  if (context) {
    const parts = context.split(".");
    for (let i = parts.length; i > 0; i--) {
      const prefix = parts.slice(0, i).join(".");
      const fqn = `${prefix}.${typeName}`;
      if (typeRegistry.has(fqn)) return typeRegistry.get(fqn);
    }
  }
  // Fallback: find any registered type ending with .TypeName
  for (const [fqn, def] of typeRegistry) {
    if (fqn === typeName || fqn.endsWith(`.${typeName}`)) return def;
  }
  return null;
}

// Convert BigInt to a JavaScript value. For int64/uint64/sint64 types the
// protobuf JSON mapping always uses strings, matching the Rust serde
// serializer. For fixed32/uint32 etc. use Number.
function bigintToNumber(v, asString = false) {
  if (asString) {
    return v.toString();
  }
  if (v >= -9007199254740991n && v <= 9007199254740991n) {
    return Number(v);
  }
  return v.toString();
}

// Fields where bytes should be rendered as hex strings
const BYTES_AS_HEX = new Set(["traceId", "spanId", "parentSpanId"]);

// Proto types that use fixed-width wire encoding
const FIXED64_TYPES = new Set(["fixed64", "sfixed64", "double"]);
const FIXED32_TYPES = new Set(["fixed32", "sfixed32", "float"]);
const INT64_TYPES = new Set([
  "int64",
  "uint64",
  "sint64",
  "fixed64",
  "sfixed64",
]);
// Types that the Rust serde serializer outputs as JSON strings (matching the
// custom `serialize_to_value` in opentelemetry-proto for AnyValue.intValue).
const INT64_STRING_TYPES = new Set(["int64", "sint64"]);

export function decode(typeName, buf) {
  const typeDef = typeRegistry.get(typeName);
  if (!typeDef) throw new Error(`unknown type: ${typeName}`);
  return decodeMessage(typeDef, typeName, new Reader(buf));
}

function decodeMessage(typeDef, typeName, reader) {
  const fields = typeDef.fields;
  const result = {};
  const repeatedFields = {};

  // Build field number -> field info map
  const fieldMap = {};
  for (const [name, field] of Object.entries(fields)) {
    fieldMap[field.id] = { name, ...field };
  }

  // Determine which fields belong to a oneof. Synthetic oneofs (proto3
  // optional fields) use a name starting with "_" and should be treated as
  // regular fields — prost/serde serializes them inline.
  const oneofFieldNames = new Set();
  if (typeDef.oneofs) {
    for (const [oneofName, oneof] of Object.entries(typeDef.oneofs)) {
      if (oneofName.startsWith("_")) continue;
      for (const fieldName of oneof.oneof) {
        oneofFieldNames.add(fieldName);
      }
    }
  }

  while (reader.pos < reader.end) {
    const tag = Number(reader.varint());
    const fieldNum = tag >>> 3;
    const wireType = tag & 7;
    const fieldInfo = fieldMap[fieldNum];

    if (!fieldInfo) {
      reader.skip(wireType);
      continue;
    }

    let value;
    const fieldType = fieldInfo.type;
    const resolvedType = resolveType(fieldType, typeName);

    switch (wireType) {
      case LEN: {
        if (resolvedType) {
          // Embedded message
          const len = Number(reader.varint());
          const subReader = new Reader(
            reader.buf,
            reader.pos,
            reader.pos + len,
          );
          reader.pos += len;
          value = decodeMessage(resolvedType, fieldType, subReader);
        } else if (fieldType === "string") {
          const len = Number(reader.varint());
          value = new TextDecoder().decode(
            reader.buf.slice(reader.pos, reader.pos + len),
          );
          reader.pos += len;
        } else if (fieldType === "bytes") {
          const len = Number(reader.varint());
          const data = reader.buf.slice(reader.pos, reader.pos + len);
          reader.pos += len;
          value = bytesToHex(data);
        } else if (fieldInfo.rule === "repeated") {
          // Packed repeated scalar field
          const len = Number(reader.varint());
          const end = reader.pos + len;
          if (!repeatedFields[fieldInfo.name]) {
            repeatedFields[fieldInfo.name] = [];
          }
          while (reader.pos < end) {
            let v;
            if (fieldType === "double") {
              v = reader.float64();
            } else if (fieldType === "float") {
              v = reader.float32();
            } else if (
              FIXED64_TYPES.has(fieldType)
            ) {
              const raw = reader.fixed64();
              v = bigintToNumber(raw, INT64_STRING_TYPES.has(fieldType));
            } else if (FIXED32_TYPES.has(fieldType)) {
              v = reader.fixed32();
            } else {
              const raw = reader.varint();
              v = bigintToNumber(raw, INT64_STRING_TYPES.has(fieldType));
            }
            repeatedFields[fieldInfo.name].push(v);
          }
          continue;
        } else {
          // Unknown length-delimited - skip
          const len = Number(reader.varint());
          reader.pos += len;
          continue;
        }
        break;
      }
      case VARINT: {
        const v = reader.varint();
        if (fieldType === "bool") {
          value = v !== 0n;
        } else if (INT64_TYPES.has(fieldType)) {
          value = bigintToNumber(v, INT64_STRING_TYPES.has(fieldType));
        } else {
          value = Number(v);
        }
        break;
      }
      case I64: {
        if (fieldType === "double") {
          value = reader.float64();
        } else {
          const v = reader.fixed64();
          value = bigintToNumber(v, INT64_STRING_TYPES.has(fieldType));
        }
        break;
      }
      case I32: {
        if (fieldType === "float") {
          value = reader.float32();
        } else {
          value = reader.fixed32();
        }
        break;
      }
      default:
        reader.skip(wireType);
        continue;
    }

    if (fieldInfo.rule === "repeated") {
      if (!repeatedFields[fieldInfo.name]) {
        repeatedFields[fieldInfo.name] = [];
      }
      repeatedFields[fieldInfo.name].push(value);
    } else {
      result[fieldInfo.name] = value;
    }
  }

  // Build output matching the Rust/serde JSON field order: regular fields in
  // proto definition order first, then any active oneof field.
  const ordered = {};
  const fieldEntries = Object.entries(fields);

  // First pass: non-oneof fields in definition order
  for (const [name, field] of fieldEntries) {
    if (oneofFieldNames.has(name)) continue;

    if (field.rule === "repeated") {
      ordered[name] = repeatedFields[name] || [];
    } else if (name in result) {
      ordered[name] = result[name];
    } else {
      const rt = resolveType(field.type, typeName);
      if (rt) {
        ordered[name] = null;
      } else if (field.type === "string" || field.type === "bytes") {
        ordered[name] = "";
      } else if (field.type === "bool") {
        ordered[name] = false;
      } else if (INT64_STRING_TYPES.has(field.type)) {
        ordered[name] = "0";
      } else {
        ordered[name] = 0;
      }
    }
  }

  // Second pass: active oneof fields
  for (const [name, _field] of fieldEntries) {
    if (!oneofFieldNames.has(name)) continue;
    if (name in result) ordered[name] = result[name];
    else if (name in repeatedFields) ordered[name] = repeatedFields[name];
  }

  return ordered;
}

export function decodeExportTraceRequest(buf) {
  return decode(
    "opentelemetry.proto.collector.trace.v1.ExportTraceServiceRequest",
    buf,
  );
}

export function decodeExportMetricsRequest(buf) {
  return decode(
    "opentelemetry.proto.collector.metrics.v1.ExportMetricsServiceRequest",
    buf,
  );
}

export function decodeExportLogsRequest(buf) {
  return decode(
    "opentelemetry.proto.collector.logs.v1.ExportLogsServiceRequest",
    buf,
  );
}
