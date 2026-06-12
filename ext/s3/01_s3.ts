// Copyright 2018-2026 the Deno authors. MIT license.

// Implementation of the unstable `Deno.S3Client` and `Deno.s3` APIs: a
// built-in client for S3-compatible object storage.
//
// Everything is implemented in JavaScript on top of `fetch` and WebCrypto:
// requests are signed with AWS Signature Version 4 (header-based auth for
// requests, query-based auth for presigned URLs). Multipart uploads are used
// for streaming writes via `S3File.prototype.writer`.

(function () {
const { core, primordials } = __bootstrap;
const {
  ArrayBufferIsView,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeSort,
  DataViewPrototype,
  DataViewPrototypeGetBuffer,
  DataViewPrototypeGetByteLength,
  DataViewPrototypeGetByteOffset,
  Date,
  DatePrototypeToISOString,
  Error,
  MathMin,
  NumberParseInt,
  NumberPrototypeToString,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  RangeError,
  RegExpPrototypeExec,
  SafeArrayIterator,
  SafeMap,
  SafeRegExp,
  StringFromCharCode,
  StringPrototypeEndsWith,
  StringPrototypeIndexOf,
  StringPrototypePadStart,
  StringPrototypeReplace,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  StringPrototypeToLowerCase,
  Symbol,
  TypeError,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeSubarray,
  Uint8Array,
  Uint8ArrayPrototype,
} = primordials;

const { TextEncoder } = core.loadExtScript(
  "ext:deno_web/08_text_encoding.js",
);
const { crypto } = core.loadExtScript("ext:deno_crypto/00_crypto.js");
const { URL } = core.loadExtScript("ext:deno_web/00_url.js");
const { env } = core.loadExtScript("ext:deno_os/30_os.js");

// fetch (and the web streams it depends on) are loaded lazily so that simply
// constructing a client does not pull in the heavy fetch/streams machinery.
let _fetch: ((input: string, init: unknown) => Promise<Response>) | undefined;
function lazyFetch() {
  return _fetch ??
    (_fetch = core.loadExtScript("ext:deno_fetch/26_fetch.js").fetch);
}
let _streams;
function lazyStreams() {
  return _streams ??
    (_streams = core.loadExtScript("ext:deno_web/06_streams.js"));
}
let _file;
function lazyFile() {
  return _file ?? (_file = core.loadExtScript("ext:deno_web/09_file.js"));
}

const encoder = new TextEncoder();

const EMPTY_PAYLOAD_SHA256 =
  "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const UNSIGNED_PAYLOAD = "UNSIGNED-PAYLOAD";

// Minimum/default part size for multipart uploads (S3 requires every part
// except the last one to be at least 5 MiB).
const MIN_PART_SIZE = 5 * 1024 * 1024;
const MAX_PART_SIZE = 5 * 1024 * 1024 * 1024;

const constructKey = Symbol("constructKey");

// === small utilities ===

function bytesToHex(bytes: Uint8Array): string {
  let out = "";
  const len = TypedArrayPrototypeGetLength(bytes);
  for (let i = 0; i < len; i++) {
    out += StringPrototypePadStart(
      NumberPrototypeToString(bytes[i], 16),
      2,
      "0",
    );
  }
  return out;
}

function concatBytes(chunks: Uint8Array[], total: number): Uint8Array {
  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of new SafeArrayIterator(chunks)) {
    TypedArrayPrototypeSet(out, chunk, offset);
    offset += TypedArrayPrototypeGetLength(chunk);
  }
  return out;
}

function isUnreservedByte(b: number): boolean {
  return (b >= 0x41 && b <= 0x5a) || // A-Z
    (b >= 0x61 && b <= 0x7a) || // a-z
    (b >= 0x30 && b <= 0x39) || // 0-9
    b === 0x2d || b === 0x2e || b === 0x5f || b === 0x7e; // - . _ ~
}

// AWS-style URI encoding: percent-encode everything except unreserved
// characters (and `/` when `encodeSlash` is false), using uppercase hex.
function uriEncode(value: string, encodeSlash: boolean): string {
  const bytes = encoder.encode(value);
  const len = TypedArrayPrototypeGetLength(bytes);
  let out = "";
  for (let i = 0; i < len; i++) {
    const b = bytes[i];
    if (isUnreservedByte(b) || (b === 0x2f && !encodeSlash)) {
      out += StringFromCharCode(b);
    } else {
      out += "%" + toUpperHex(b);
    }
  }
  return out;
}

function toUpperHex(b: number): string {
  const HEX = "0123456789ABCDEF";
  return HEX[(b >> 4) & 0xf] + HEX[b & 0xf];
}

function amzTimestamp(): string {
  // 2026-06-12T15:04:05.123Z -> 20260612T150405Z
  const iso = DatePrototypeToISOString(new Date());
  return StringPrototypeReplace(
    StringPrototypeSlice(iso, 0, 19),
    DATE_SEPARATORS_RE,
    "",
  ) + "Z";
}
const DATE_SEPARATORS_RE = new SafeRegExp(/[-:]/g);

async function sha256Hex(data: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", data);
  return bytesToHex(new Uint8Array(digest));
}

async function hmacSha256(
  key: Uint8Array,
  data: string,
): Promise<Uint8Array> {
  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    key,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign(
    "HMAC",
    cryptoKey,
    encoder.encode(data),
  );
  return new Uint8Array(signature);
}

// Cache of derived SigV4 signing keys, keyed by date/region/secret.
const signingKeyCache = new SafeMap();

async function getSigningKey(
  secretAccessKey: string,
  date: string,
  region: string,
): Promise<Uint8Array> {
  const cacheKey = `${date}/${region}/${secretAccessKey}`;
  const cached = signingKeyCache.get(cacheKey);
  if (cached !== undefined) return cached;
  let key = await hmacSha256(encoder.encode("AWS4" + secretAccessKey), date);
  key = await hmacSha256(key, region);
  key = await hmacSha256(key, "s3");
  key = await hmacSha256(key, "aws4_request");
  if (signingKeyCache.size > 32) signingKeyCache.clear();
  signingKeyCache.set(cacheKey, key);
  return key;
}

// === minimal XML helpers (S3 responses are simple, flat XML) ===

const xmlTagRegexCache = new SafeMap();
function xmlTagRegex(tag: string): RegExp {
  let re = xmlTagRegexCache.get(tag);
  if (re === undefined) {
    re = new SafeRegExp(`<${tag}>([\\s\\S]*?)</${tag}>`, "g");
    xmlTagRegexCache.set(tag, re);
  }
  re.lastIndex = 0;
  return re;
}

function decodeXmlEntities(value: string): string {
  return StringPrototypeReplace(
    value,
    XML_ENTITY_RE,
    (entity: string) => {
      switch (entity) {
        case "&amp;":
          return "&";
        case "&lt;":
          return "<";
        case "&gt;":
          return ">";
        case "&quot;":
          return '"';
        case "&apos;":
          return "'";
        default: {
          // numeric entity: &#123; or &#x1F;
          const body = StringPrototypeSlice(entity, 2, -1);
          const code = StringPrototypeStartsWith(body, "x") ||
              StringPrototypeStartsWith(body, "X")
            ? NumberParseInt(StringPrototypeSlice(body, 1), 16)
            : NumberParseInt(body, 10);
          return StringFromCharCode(code);
        }
      }
    },
  );
}
const XML_ENTITY_RE = new SafeRegExp(
  /&(?:amp|lt|gt|quot|apos|#x?[0-9a-fA-F]+);/g,
);

function xmlText(xml: string, tag: string): string | undefined {
  const match = RegExpPrototypeExec(xmlTagRegex(tag), xml);
  if (match === null) return undefined;
  return decodeXmlEntities(match[1]);
}

function xmlBlocks(xml: string, tag: string): string[] {
  const re = xmlTagRegex(tag);
  const out: string[] = [];
  let match;
  while ((match = RegExpPrototypeExec(re, xml)) !== null) {
    ArrayPrototypePush(out, match[1]);
  }
  return out;
}

// === errors ===

class S3Error extends Error {
  code: string;
  status: number;
  bucket?: string;
  key?: string;

  constructor(
    code: string,
    message: string,
    status: number,
    bucket?: string,
    key?: string,
  ) {
    super(message);
    this.name = "S3Error";
    this.code = code;
    this.status = status;
    this.bucket = bucket;
    this.key = key;
  }
}

async function throwS3Error(
  resp: Response,
  bucket: string,
  key?: string,
): Promise<never> {
  let code: string | undefined;
  let message: string | undefined;
  try {
    const text = await resp.text();
    code = xmlText(text, "Code");
    message = xmlText(text, "Message");
  } catch {
    // ignore body read errors, fall back to status-based message
  }
  if (code === undefined) {
    switch (resp.status) {
      case 404:
        code = "NoSuchKey";
        break;
      case 403:
        code = "AccessDenied";
        break;
      default:
        code = "UnknownError";
    }
  }
  message ??= `S3 request failed with status ${resp.status}`;
  throw new S3Error(code, message, resp.status, bucket, key);
}

// === configuration ===

function envGet(name: string): string | undefined {
  let value;
  try {
    value = env.get(name);
  } catch (error) {
    // Environment variables are only a fallback for options that were not
    // passed explicitly: without --allow-env, behave as if they are unset
    // instead of failing client construction.
    if ((error as Error)?.name === "NotCapable") {
      return undefined;
    }
    throw error;
  }
  return value === "" ? undefined : value;
}

interface ResolvedConfig {
  accessKeyId?: string;
  secretAccessKey?: string;
  sessionToken?: string;
  region: string;
  bucket?: string;
  protocol: string;
  host: string;
  pathPrefix: string;
  virtualHostedStyle: boolean;
  acl?: string;
  storageClass?: string;
}

function resolveConfig(options): ResolvedConfig {
  const accessKeyId = options.accessKeyId ?? envGet("S3_ACCESS_KEY_ID") ??
    envGet("AWS_ACCESS_KEY_ID");
  const secretAccessKey = options.secretAccessKey ??
    envGet("S3_SECRET_ACCESS_KEY") ?? envGet("AWS_SECRET_ACCESS_KEY");
  const sessionToken = options.sessionToken ?? envGet("S3_SESSION_TOKEN") ??
    envGet("AWS_SESSION_TOKEN");
  const region = options.region ?? envGet("S3_REGION") ??
    envGet("AWS_REGION") ?? envGet("AWS_DEFAULT_REGION") ?? "us-east-1";
  const bucket = options.bucket ?? envGet("S3_BUCKET") ?? envGet("AWS_BUCKET");
  const endpoint = options.endpoint ?? envGet("S3_ENDPOINT") ??
    envGet("AWS_ENDPOINT") ?? `https://s3.${region}.amazonaws.com`;

  if ((accessKeyId === undefined) !== (secretAccessKey === undefined)) {
    throw new TypeError(
      "Both `accessKeyId` and `secretAccessKey` must be provided (or neither, for anonymous access)",
    );
  }

  let url;
  try {
    url = new URL(endpoint);
  } catch {
    throw new TypeError(`Invalid S3 endpoint: ${endpoint}`);
  }
  // Strip a trailing slash so the prefix can be joined with `/bucket/key`.
  // A bare "/" pathname becomes "".
  let pathPrefix = url.pathname;
  if (StringPrototypeEndsWith(pathPrefix, "/")) {
    pathPrefix = StringPrototypeSlice(pathPrefix, 0, pathPrefix.length - 1);
  }

  return {
    accessKeyId,
    secretAccessKey,
    sessionToken,
    region,
    bucket,
    protocol: url.protocol,
    host: url.host,
    pathPrefix,
    virtualHostedStyle: options.virtualHostedStyle ?? false,
    acl: options.acl,
    storageClass: options.storageClass,
  };
}

function parsePath(
  path: string,
  config: ResolvedConfig,
  options,
): { bucket: string; key: string } {
  if (typeof path !== "string" || path.length === 0) {
    throw new TypeError("S3 object path must be a non-empty string");
  }
  if (StringPrototypeStartsWith(path, "s3://")) {
    const rest = StringPrototypeSlice(path, 5);
    const slash = StringPrototypeIndexOf(rest, "/");
    if (slash <= 0 || slash === rest.length - 1) {
      throw new TypeError(`Invalid s3:// URL: ${path}`);
    }
    return {
      bucket: StringPrototypeSlice(rest, 0, slash),
      key: StringPrototypeSlice(rest, slash + 1),
    };
  }
  let key = path;
  if (StringPrototypeStartsWith(key, "/")) {
    key = StringPrototypeSlice(key, 1);
  }
  const bucket = options?.bucket ?? config.bucket;
  if (bucket === undefined) {
    throw new TypeError(
      "Missing S3 bucket name: pass `bucket` in the client or per-call options, set the S3_BUCKET environment variable, or use an `s3://bucket/key` path",
    );
  }
  return { bucket, key };
}

// === request signing and execution ===

function buildHostAndPath(
  config: ResolvedConfig,
  bucket: string,
  key: string,
): { host: string; canonicalPath: string } {
  if (config.virtualHostedStyle) {
    return {
      host: `${bucket}.${config.host}`,
      canonicalPath: `${config.pathPrefix}/${uriEncode(key, false)}`,
    };
  }
  const keyPart = key === "" ? "" : `/${uriEncode(key, false)}`;
  return {
    host: config.host,
    canonicalPath: `${config.pathPrefix}/${uriEncode(bucket, true)}${keyPart}`,
  };
}

function canonicalQueryString(query: string[][]): string {
  const encoded = ArrayPrototypeMap(
    query,
    (pair) => [uriEncode(pair[0], true), uriEncode(pair[1], true)],
  );
  ArrayPrototypeSort(encoded, (a, b) => {
    const ka = a[0];
    const kb = b[0];
    if (ka < kb) return -1;
    if (ka > kb) return 1;
    return a[1] < b[1] ? -1 : a[1] > b[1] ? 1 : 0;
  });
  return ArrayPrototypeJoin(
    ArrayPrototypeMap(encoded, (pair) => `${pair[0]}=${pair[1]}`),
    "&",
  );
}

interface S3RequestOptions {
  query?: string[][];
  headers?: Record<string, string>;
  body?: Uint8Array | null;
  payloadHash?: string;
}

async function s3Fetch(
  config: ResolvedConfig,
  method: string,
  bucket: string,
  key: string,
  { query = [], headers = { __proto__: null }, body = null, payloadHash }:
    S3RequestOptions = { __proto__: null },
): Promise<Response> {
  const { host, canonicalPath } = buildHostAndPath(config, bucket, key);
  const queryString = canonicalQueryString(query);
  const url = `${config.protocol}//${host}${canonicalPath}` +
    (queryString === "" ? "" : `?${queryString}`);

  if (payloadHash === undefined) {
    payloadHash = body === null ? EMPTY_PAYLOAD_SHA256 : await sha256Hex(body);
  }

  const requestHeaders: Record<string, string> = { ...headers };

  if (config.accessKeyId !== undefined) {
    const timestamp = amzTimestamp();
    const date = StringPrototypeSlice(timestamp, 0, 8);
    const scope = `${date}/${config.region}/s3/aws4_request`;

    const signedHeaderEntries: string[][] = [["host", host]];
    for (const name of new SafeArrayIterator(ObjectKeys(headers))) {
      ArrayPrototypePush(signedHeaderEntries, [
        StringPrototypeToLowerCase(name),
        headers[name],
      ]);
    }
    ArrayPrototypePush(signedHeaderEntries, [
      "x-amz-content-sha256",
      payloadHash,
    ]);
    ArrayPrototypePush(signedHeaderEntries, ["x-amz-date", timestamp]);
    if (config.sessionToken !== undefined) {
      ArrayPrototypePush(signedHeaderEntries, [
        "x-amz-security-token",
        config.sessionToken,
      ]);
    }
    ArrayPrototypeSort(
      signedHeaderEntries,
      (a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0,
    );

    const canonicalHeaders = ArrayPrototypeJoin(
      ArrayPrototypeMap(
        signedHeaderEntries,
        (pair) => `${pair[0]}:${pair[1]}\n`,
      ),
      "",
    );
    const signedHeaderNames = ArrayPrototypeJoin(
      ArrayPrototypeMap(signedHeaderEntries, (pair) => pair[0]),
      ";",
    );

    const canonicalRequest =
      `${method}\n${canonicalPath}\n${queryString}\n${canonicalHeaders}\n${signedHeaderNames}\n${payloadHash}`;
    const stringToSign =
      `AWS4-HMAC-SHA256\n${timestamp}\n${scope}\n${await sha256Hex(
        encoder.encode(canonicalRequest),
      )}`;
    const signingKey = await getSigningKey(
      config.secretAccessKey,
      date,
      config.region,
    );
    const signature = bytesToHex(await hmacSha256(signingKey, stringToSign));

    requestHeaders["x-amz-content-sha256"] = payloadHash;
    requestHeaders["x-amz-date"] = timestamp;
    if (config.sessionToken !== undefined) {
      requestHeaders["x-amz-security-token"] = config.sessionToken;
    }
    requestHeaders["authorization"] =
      `AWS4-HMAC-SHA256 Credential=${config.accessKeyId}/${scope}, SignedHeaders=${signedHeaderNames}, Signature=${signature}`;
  }

  const fetch = lazyFetch();
  return await fetch(url, {
    method,
    headers: requestHeaders,
    body,
  });
}

async function drainBody(resp: Response): Promise<void> {
  try {
    if (resp.body !== null) {
      await resp.body.cancel();
    }
  } catch {
    // already consumed or errored, nothing to release
  }
}

// === multipart upload primitives ===

async function createMultipartUpload(
  config: ResolvedConfig,
  bucket: string,
  key: string,
  headers: Record<string, string>,
): Promise<string> {
  const resp = await s3Fetch(config, "POST", bucket, key, {
    query: [["uploads", ""]],
    headers,
  });
  if (!resp.ok) await throwS3Error(resp, bucket, key);
  const text = await resp.text();
  const uploadId = xmlText(text, "UploadId");
  if (uploadId === undefined) {
    throw new S3Error(
      "InvalidResponse",
      "CreateMultipartUpload response did not contain an UploadId",
      resp.status,
      bucket,
      key,
    );
  }
  return uploadId;
}

async function uploadPart(
  config: ResolvedConfig,
  bucket: string,
  key: string,
  uploadId: string,
  partNumber: number,
  body: Uint8Array,
): Promise<string> {
  const resp = await s3Fetch(config, "PUT", bucket, key, {
    query: [
      ["partNumber", NumberPrototypeToString(partNumber, 10)],
      ["uploadId", uploadId],
    ],
    body,
  });
  if (!resp.ok) await throwS3Error(resp, bucket, key);
  const etag = resp.headers.get("etag");
  await drainBody(resp);
  if (etag === null) {
    throw new S3Error(
      "InvalidResponse",
      "UploadPart response did not contain an ETag",
      resp.status,
      bucket,
      key,
    );
  }
  return etag;
}

async function completeMultipartUpload(
  config: ResolvedConfig,
  bucket: string,
  key: string,
  uploadId: string,
  parts: { partNumber: number; etag: string }[],
): Promise<void> {
  let xml = '<?xml version="1.0" encoding="UTF-8"?><CompleteMultipartUpload>';
  for (const part of new SafeArrayIterator(parts)) {
    xml +=
      `<Part><PartNumber>${part.partNumber}</PartNumber><ETag>${part.etag}</ETag></Part>`;
  }
  xml += "</CompleteMultipartUpload>";
  const resp = await s3Fetch(config, "POST", bucket, key, {
    query: [["uploadId", uploadId]],
    headers: { "content-type": "application/xml" },
    body: encoder.encode(xml),
  });
  if (!resp.ok) await throwS3Error(resp, bucket, key);
  // A 200 response can still contain an error document.
  const text = await resp.text();
  const code = xmlText(text, "Code");
  if (code !== undefined && xmlText(text, "ETag") === undefined) {
    throw new S3Error(
      code,
      xmlText(text, "Message") ?? "CompleteMultipartUpload failed",
      resp.status,
      bucket,
      key,
    );
  }
}

async function abortMultipartUpload(
  config: ResolvedConfig,
  bucket: string,
  key: string,
  uploadId: string,
): Promise<void> {
  const resp = await s3Fetch(config, "DELETE", bucket, key, {
    query: [["uploadId", uploadId]],
  });
  await drainBody(resp);
}

// === write helpers ===

function writeHeaders(
  config: ResolvedConfig,
  options,
  defaultType?: string,
): Record<string, string> {
  const headers: Record<string, string> = {
    "content-type": options?.type ?? defaultType ?? "application/octet-stream",
  };
  const acl = options?.acl ?? config.acl;
  if (acl !== undefined) headers["x-amz-acl"] = acl;
  const storageClass = options?.storageClass ?? config.storageClass;
  if (storageClass !== undefined) {
    headers["x-amz-storage-class"] = storageClass;
  }
  return headers;
}

function isBlob(value: unknown): boolean {
  const { BlobPrototype } = lazyFile();
  return ObjectPrototypeIsPrototypeOf(BlobPrototype, value);
}

function toBytes(data: unknown): Uint8Array | null {
  if (typeof data === "string") {
    return encoder.encode(data);
  }
  if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, data)) {
    return data as Uint8Array;
  }
  if (ArrayBufferIsView(data)) {
    if (ObjectPrototypeIsPrototypeOf(DataViewPrototype, data)) {
      return new Uint8Array(
        DataViewPrototypeGetBuffer(data as DataView),
        DataViewPrototypeGetByteOffset(data as DataView),
        DataViewPrototypeGetByteLength(data as DataView),
      );
    }
    return new Uint8Array(
      TypedArrayPrototypeGetBuffer(data),
      TypedArrayPrototypeGetByteOffset(data),
      TypedArrayPrototypeGetByteLength(data),
    );
  }
  if (
    typeof data === "object" && data !== null &&
    // deno-lint-ignore prefer-primordials
    typeof (data as ArrayBuffer).byteLength === "number" &&
    !ArrayBufferIsView(data)
  ) {
    return new Uint8Array(data as ArrayBuffer);
  }
  return null;
}

async function putObject(
  config: ResolvedConfig,
  bucket: string,
  key: string,
  body: Uint8Array,
  headers: Record<string, string>,
): Promise<number> {
  const resp = await s3Fetch(config, "PUT", bucket, key, { headers, body });
  if (!resp.ok) await throwS3Error(resp, bucket, key);
  await drainBody(resp);
  return TypedArrayPrototypeGetLength(body);
}

// Writes arbitrary supported data to an object. Streams and large blobs go
// through the multipart writer; everything else is a single PUT.
async function writeData(
  config: ResolvedConfig,
  bucket: string,
  key: string,
  data: unknown,
  options,
): Promise<number> {
  if (ObjectPrototypeIsPrototypeOf(S3FilePrototype, data)) {
    const bytes = await (data as S3File).bytes();
    return await putObject(
      config,
      bucket,
      key,
      bytes,
      writeHeaders(config, options),
    );
  }
  if (
    typeof data === "object" && data !== null &&
    // deno-lint-ignore no-explicit-any
    typeof (data as any).getReader === "function"
  ) {
    // ReadableStream: stream through a multipart writer.
    const writer = new S3Writer(constructKey, config, bucket, key, options);
    // deno-lint-ignore no-explicit-any
    const reader = (data as any).getReader();
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      await writer.write(value);
    }
    return await writer.end();
  }
  if (isBlob(data)) {
    // deno-lint-ignore no-explicit-any
    const blob = data as any;
    const bytes = new Uint8Array(await blob.arrayBuffer());
    return await putObject(
      config,
      bucket,
      key,
      bytes,
      writeHeaders(config, options, blob.type === "" ? undefined : blob.type),
    );
  }
  const bytes = toBytes(data);
  if (bytes === null) {
    throw new TypeError(
      "Unsupported data type for S3 write: expected string, ArrayBuffer, TypedArray, Blob, ReadableStream, or S3File",
    );
  }
  return await putObject(
    config,
    bucket,
    key,
    bytes,
    writeHeaders(config, options),
  );
}

// === stat / presign helpers ===

async function statObject(
  config: ResolvedConfig,
  bucket: string,
  key: string,
) {
  const resp = await s3Fetch(config, "HEAD", bucket, key);
  await drainBody(resp);
  if (!resp.ok) {
    if (resp.status === 404) {
      throw new S3Error(
        "NoSuchKey",
        "The specified key does not exist.",
        404,
        bucket,
        key,
      );
    }
    // HEAD responses have no body, so synthesize the error from the status.
    throw new S3Error(
      resp.status === 403 ? "AccessDenied" : "UnknownError",
      `S3 HEAD request failed with status ${resp.status}`,
      resp.status,
      bucket,
      key,
    );
  }
  const size = NumberParseInt(resp.headers.get("content-length") ?? "0", 10);
  const lastModifiedHeader = resp.headers.get("last-modified");
  return {
    size,
    etag: resp.headers.get("etag") ?? undefined,
    lastModified: lastModifiedHeader === null
      ? undefined
      : new Date(lastModifiedHeader),
    type: resp.headers.get("content-type") ?? "application/octet-stream",
  };
}

async function presignUrl(
  config: ResolvedConfig,
  bucket: string,
  key: string,
  options,
): Promise<string> {
  if (config.accessKeyId === undefined) {
    throw new TypeError(
      "Cannot presign a URL without credentials: provide `accessKeyId` and `secretAccessKey`",
    );
  }
  const method = options?.method ?? "GET";
  const expiresIn = options?.expiresIn ?? 86400;
  if (!(expiresIn > 0 && expiresIn <= 604800)) {
    throw new RangeError(
      "`expiresIn` must be between 1 and 604800 seconds (7 days)",
    );
  }

  const { host, canonicalPath } = buildHostAndPath(config, bucket, key);
  const timestamp = amzTimestamp();
  const date = StringPrototypeSlice(timestamp, 0, 8);
  const scope = `${date}/${config.region}/s3/aws4_request`;

  const query: string[][] = [
    ["X-Amz-Algorithm", "AWS4-HMAC-SHA256"],
    ["X-Amz-Credential", `${config.accessKeyId}/${scope}`],
    ["X-Amz-Date", timestamp],
    ["X-Amz-Expires", NumberPrototypeToString(expiresIn, 10)],
    ["X-Amz-SignedHeaders", "host"],
  ];
  if (config.sessionToken !== undefined) {
    ArrayPrototypePush(query, ["X-Amz-Security-Token", config.sessionToken]);
  }
  if (options?.acl !== undefined) {
    ArrayPrototypePush(query, ["X-Amz-Acl", options.acl]);
  }

  const queryString = canonicalQueryString(query);
  const canonicalRequest =
    `${method}\n${canonicalPath}\n${queryString}\nhost:${host}\n\nhost\n${UNSIGNED_PAYLOAD}`;
  const stringToSign =
    `AWS4-HMAC-SHA256\n${timestamp}\n${scope}\n${await sha256Hex(
      encoder.encode(canonicalRequest),
    )}`;
  const signingKey = await getSigningKey(
    config.secretAccessKey,
    date,
    config.region,
  );
  const signature = bytesToHex(await hmacSha256(signingKey, stringToSign));

  return `${config.protocol}//${host}${canonicalPath}?${queryString}&X-Amz-Signature=${signature}`;
}

// === S3Writer: incremental (multipart) upload ===

class S3Writer {
  #config: ResolvedConfig;
  #bucket: string;
  #key: string;
  #options;
  #partSize: number;
  #chunks: Uint8Array[] = [];
  #buffered = 0;
  #bytesWritten = 0;
  #uploadId: string | null = null;
  #parts: { partNumber: number; etag: string }[] = [];
  #nextPartNumber = 1;
  #done = false;

  constructor(key: symbol, config, bucket, objectKey, options) {
    if (key !== constructKey) {
      throw new TypeError("S3Writer can not be constructed directly");
    }
    this.#config = config;
    this.#bucket = bucket;
    this.#key = objectKey;
    this.#options = options;
    let partSize = options?.partSize ?? MIN_PART_SIZE;
    if (partSize < MIN_PART_SIZE) partSize = MIN_PART_SIZE;
    if (partSize > MAX_PART_SIZE) partSize = MAX_PART_SIZE;
    this.#partSize = partSize;
  }

  async write(chunk: unknown): Promise<number> {
    if (this.#done) {
      throw new TypeError("Cannot write to a finished S3 writer");
    }
    const bytes = toBytes(chunk);
    if (bytes === null) {
      throw new TypeError(
        "Unsupported chunk type for S3 writer: expected string, ArrayBuffer, or TypedArray",
      );
    }
    const len = TypedArrayPrototypeGetLength(bytes);
    if (len > 0) {
      // Copy: the caller may reuse the buffer before we upload it.
      ArrayPrototypePush(this.#chunks, new Uint8Array(bytes));
      this.#buffered += len;
      this.#bytesWritten += len;
    }
    if (this.#buffered >= this.#partSize) {
      await this.#uploadBufferedParts();
    }
    return len;
  }

  async flush(): Promise<void> {
    if (this.#buffered >= this.#partSize) {
      await this.#uploadBufferedParts();
    }
  }

  async end(): Promise<number> {
    if (this.#done) return this.#bytesWritten;
    this.#done = true;
    try {
      if (this.#uploadId === null) {
        // Everything fit in the buffer: a single PUT is enough.
        const body = concatBytes(this.#chunks, this.#buffered);
        this.#chunks = [];
        this.#buffered = 0;
        await putObject(
          this.#config,
          this.#bucket,
          this.#key,
          body,
          writeHeaders(this.#config, this.#options),
        );
        return this.#bytesWritten;
      }
      // Upload the remainder as the (possibly undersized) final part.
      if (this.#buffered > 0) {
        const body = concatBytes(this.#chunks, this.#buffered);
        this.#chunks = [];
        this.#buffered = 0;
        await this.#uploadPart(body);
      }
      await completeMultipartUpload(
        this.#config,
        this.#bucket,
        this.#key,
        this.#uploadId,
        this.#parts,
      );
      return this.#bytesWritten;
    } catch (error) {
      if (this.#uploadId !== null) {
        try {
          await abortMultipartUpload(
            this.#config,
            this.#bucket,
            this.#key,
            this.#uploadId,
          );
        } catch {
          // best effort cleanup
        }
      }
      throw error;
    }
  }

  async abort(): Promise<void> {
    this.#done = true;
    this.#chunks = [];
    this.#buffered = 0;
    if (this.#uploadId !== null) {
      await abortMultipartUpload(
        this.#config,
        this.#bucket,
        this.#key,
        this.#uploadId,
      );
      this.#uploadId = null;
    }
  }

  async #uploadBufferedParts(): Promise<void> {
    let buffer = concatBytes(this.#chunks, this.#buffered);
    this.#chunks = [];
    this.#buffered = 0;
    while (TypedArrayPrototypeGetLength(buffer) >= this.#partSize) {
      const part = TypedArrayPrototypeSubarray(buffer, 0, this.#partSize);
      buffer = TypedArrayPrototypeSubarray(buffer, this.#partSize);
      await this.#uploadPart(part);
    }
    const rest = TypedArrayPrototypeGetLength(buffer);
    if (rest > 0) {
      ArrayPrototypePush(this.#chunks, buffer);
      this.#buffered = rest;
    }
  }

  async #uploadPart(body: Uint8Array): Promise<void> {
    if (this.#uploadId === null) {
      this.#uploadId = await createMultipartUpload(
        this.#config,
        this.#bucket,
        this.#key,
        writeHeaders(this.#config, this.#options),
      );
    }
    const partNumber = this.#nextPartNumber++;
    const etag = await uploadPart(
      this.#config,
      this.#bucket,
      this.#key,
      this.#uploadId,
      partNumber,
      body,
    );
    ArrayPrototypePush(this.#parts, { partNumber, etag });
  }
}

// === S3File: a lazy reference to an object ===

class S3File {
  #config: ResolvedConfig;
  #bucket: string;
  #key: string;
  #start?: number;
  #end?: number;
  #type?: string;

  constructor(key: symbol, config, bucket, objectKey, options) {
    if (key !== constructKey) {
      throw new TypeError(
        "S3File can not be constructed directly: use S3Client.prototype.file",
      );
    }
    this.#config = config;
    this.#bucket = bucket;
    this.#key = objectKey;
    this.#type = options?.type;
    this.#start = options?.start;
    this.#end = options?.end;
  }

  get bucket(): string {
    return this.#bucket;
  }

  get key(): string {
    return this.#key;
  }

  get type(): string {
    return this.#type ?? "application/octet-stream";
  }

  slice(start?: number, end?: number): S3File {
    start ??= 0;
    if (start < 0 || (end !== undefined && end < 0)) {
      throw new RangeError(
        "S3File.prototype.slice does not support negative offsets",
      );
    }
    const base = this.#start ?? 0;
    const newStart = base + start;
    let newEnd: number | undefined;
    if (end !== undefined) {
      newEnd = base + end;
      if (this.#end !== undefined) newEnd = MathMin(newEnd, this.#end);
    } else {
      newEnd = this.#end;
    }
    return new S3File(constructKey, this.#config, this.#bucket, this.#key, {
      type: this.#type,
      start: newStart,
      end: newEnd,
    });
  }

  async #get(): Promise<Response> {
    const headers: Record<string, string> = {};
    if (
      this.#start !== undefined && (this.#start > 0 ||
        this.#end !== undefined)
    ) {
      const endPart = this.#end === undefined
        ? ""
        : NumberPrototypeToString(this.#end - 1, 10);
      headers["range"] = `bytes=${
        NumberPrototypeToString(this.#start, 10)
      }-${endPart}`;
    }
    const resp = await s3Fetch(this.#config, "GET", this.#bucket, this.#key, {
      headers,
    });
    if (!resp.ok) await throwS3Error(resp, this.#bucket, this.#key);
    return resp;
  }

  async arrayBuffer(): Promise<ArrayBuffer> {
    const resp = await this.#get();
    return await resp.arrayBuffer();
  }

  async bytes(): Promise<Uint8Array> {
    const resp = await this.#get();
    return new Uint8Array(await resp.arrayBuffer());
  }

  async text(): Promise<string> {
    const resp = await this.#get();
    return await resp.text();
  }

  // deno-lint-ignore no-explicit-any
  async json(): Promise<any> {
    const resp = await this.#get();
    return await resp.json();
  }

  stream(): ReadableStream<Uint8Array> {
    const { ReadableStream } = lazyStreams();
    // deno-lint-ignore no-explicit-any
    let reader: any = null;
    const getResponse = () => this.#get();
    return new ReadableStream({
      // deno-lint-ignore no-explicit-any
      async pull(controller: any) {
        if (reader === null) {
          const resp = await getResponse();
          if (resp.body === null) {
            controller.close();
            return;
          }
          reader = resp.body.getReader();
        }
        const { done, value } = await reader.read();
        if (done) {
          controller.close();
        } else {
          controller.enqueue(value);
        }
      },
      async cancel(reason: unknown) {
        if (reader !== null) {
          await reader.cancel(reason);
        }
      },
    });
  }

  async exists(): Promise<boolean> {
    try {
      await statObject(this.#config, this.#bucket, this.#key);
      return true;
    } catch (error) {
      if (
        ObjectPrototypeIsPrototypeOf(S3ErrorPrototype, error) &&
        (error as S3Error).status === 404
      ) {
        return false;
      }
      throw error;
    }
  }

  stat() {
    return statObject(this.#config, this.#bucket, this.#key);
  }

  async size(): Promise<number> {
    const info = await statObject(this.#config, this.#bucket, this.#key);
    return info.size;
  }

  write(data: unknown, options?): Promise<number> {
    return writeData(this.#config, this.#bucket, this.#key, data, {
      type: this.#type,
      ...options,
    });
  }

  writer(options?): S3Writer {
    return new S3Writer(constructKey, this.#config, this.#bucket, this.#key, {
      type: this.#type,
      ...options,
    });
  }

  async delete(): Promise<void> {
    const resp = await s3Fetch(
      this.#config,
      "DELETE",
      this.#bucket,
      this.#key,
    );
    if (!resp.ok && resp.status !== 404) {
      await throwS3Error(resp, this.#bucket, this.#key);
    }
    await drainBody(resp);
  }

  unlink(): Promise<void> {
    return this.delete();
  }

  presign(options?): Promise<string> {
    return presignUrl(this.#config, this.#bucket, this.#key, options);
  }
}
const S3FilePrototype = S3File.prototype;

const S3ErrorPrototype = S3Error.prototype;

// === S3Client ===

class S3Client {
  #options;
  #config: ResolvedConfig | null = null;

  constructor(options = { __proto__: null }) {
    if (typeof options !== "object" || options === null) {
      throw new TypeError("S3Client options must be an object");
    }
    this.#options = {
      accessKeyId: options.accessKeyId,
      secretAccessKey: options.secretAccessKey,
      sessionToken: options.sessionToken,
      region: options.region,
      bucket: options.bucket,
      endpoint: options.endpoint,
      virtualHostedStyle: options.virtualHostedStyle,
      acl: options.acl,
      storageClass: options.storageClass,
    };
  }

  // Configuration (including credentials from the environment) is resolved
  // lazily on first use so that constructing a client never prompts for
  // environment permissions.
  #resolveConfig(): ResolvedConfig {
    return this.#config ?? (this.#config = resolveConfig(this.#options));
  }

  file(path: string, options?): S3File {
    const config = this.#resolveConfig();
    const { bucket, key } = parsePath(path, config, options);
    return new S3File(constructKey, config, bucket, key, options);
  }

  write(path: string, data: unknown, options?): Promise<number> {
    const config = this.#resolveConfig();
    const { bucket, key } = parsePath(path, config, options);
    return writeData(config, bucket, key, data, options);
  }

  async delete(path: string, options?): Promise<void> {
    const config = this.#resolveConfig();
    const { bucket, key } = parsePath(path, config, options);
    const resp = await s3Fetch(config, "DELETE", bucket, key);
    if (!resp.ok && resp.status !== 404) {
      await throwS3Error(resp, bucket, key);
    }
    await drainBody(resp);
  }

  unlink(path: string, options?): Promise<void> {
    return this.delete(path, options);
  }

  async exists(path: string, options?): Promise<boolean> {
    const config = this.#resolveConfig();
    const { bucket, key } = parsePath(path, config, options);
    try {
      await statObject(config, bucket, key);
      return true;
    } catch (error) {
      if (
        ObjectPrototypeIsPrototypeOf(S3ErrorPrototype, error) &&
        (error as S3Error).status === 404
      ) {
        return false;
      }
      throw error;
    }
  }

  stat(path: string, options?) {
    const config = this.#resolveConfig();
    const { bucket, key } = parsePath(path, config, options);
    return statObject(config, bucket, key);
  }

  async size(path: string, options?): Promise<number> {
    const info = await this.stat(path, options);
    return info.size;
  }

  presign(path: string, options?): Promise<string> {
    const config = this.#resolveConfig();
    const { bucket, key } = parsePath(path, config, options);
    return presignUrl(config, bucket, key, options);
  }

  async list(options?, ctorOptions?) {
    const config = this.#resolveConfig();
    const bucket = ctorOptions?.bucket ?? options?.bucket ?? config.bucket;
    if (bucket === undefined) {
      throw new TypeError(
        "Missing S3 bucket name: pass `bucket` in the client or per-call options, or set the S3_BUCKET environment variable",
      );
    }
    const query: string[][] = [["list-type", "2"]];
    if (options?.prefix !== undefined) {
      ArrayPrototypePush(query, ["prefix", options.prefix]);
    }
    if (options?.delimiter !== undefined) {
      ArrayPrototypePush(query, ["delimiter", options.delimiter]);
    }
    if (options?.maxKeys !== undefined) {
      ArrayPrototypePush(query, [
        "max-keys",
        NumberPrototypeToString(options.maxKeys, 10),
      ]);
    }
    if (options?.continuationToken !== undefined) {
      ArrayPrototypePush(query, [
        "continuation-token",
        options.continuationToken,
      ]);
    }
    if (options?.startAfter !== undefined) {
      ArrayPrototypePush(query, ["start-after", options.startAfter]);
    }
    if (options?.fetchOwner === true) {
      ArrayPrototypePush(query, ["fetch-owner", "true"]);
    }

    const resp = await s3Fetch(config, "GET", bucket, "", { query });
    if (!resp.ok) await throwS3Error(resp, bucket);
    const xml = await resp.text();

    const contents = ArrayPrototypeMap(
      xmlBlocks(xml, "Contents"),
      (block: string) => {
        const sizeText = xmlText(block, "Size");
        return {
          key: xmlText(block, "Key"),
          lastModified: xmlText(block, "LastModified"),
          eTag: xmlText(block, "ETag"),
          size: sizeText === undefined ? 0 : NumberParseInt(sizeText, 10),
          storageClass: xmlText(block, "StorageClass"),
        };
      },
    );
    const commonPrefixes = ArrayPrototypeMap(
      xmlBlocks(xml, "CommonPrefixes"),
      (block: string) => ({ prefix: xmlText(block, "Prefix") }),
    );

    const keyCountText = xmlText(xml, "KeyCount");
    const maxKeysText = xmlText(xml, "MaxKeys");
    return {
      name: xmlText(xml, "Name"),
      prefix: xmlText(xml, "Prefix"),
      delimiter: xmlText(xml, "Delimiter"),
      startAfter: xmlText(xml, "StartAfter"),
      isTruncated: xmlText(xml, "IsTruncated") === "true",
      keyCount: keyCountText === undefined
        ? contents.length
        : NumberParseInt(keyCountText, 10),
      maxKeys: maxKeysText === undefined
        ? undefined
        : NumberParseInt(maxKeysText, 10),
      continuationToken: xmlText(xml, "ContinuationToken"),
      nextContinuationToken: xmlText(xml, "NextContinuationToken"),
      contents: contents.length === 0 ? undefined : contents,
      commonPrefixes: commonPrefixes.length === 0 ? undefined : commonPrefixes,
    };
  }
}

// The default client backing `Deno.s3`, configured entirely from
// environment variables.
let defaultClient: S3Client | undefined;
function getDefaultClient(): S3Client {
  return defaultClient ?? (defaultClient = new S3Client());
}

return { S3Client, S3File, S3Error, getDefaultClient };
})();
