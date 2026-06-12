// Copyright 2018-2026 the Deno authors. MIT license.

// ACME (RFC 8555) client used by `Deno.serve` to automatically provision and
// renew TLS certificates. Only the `http-01` challenge type is supported for
// now. The protocol layer is implemented on top of `fetch` and WebCrypto so
// that network access flows through the regular permission checks.

// TODO(prototype): web APIs (fetch, crypto, Deno.*) are accessed via
// `globalThis` for now; before landing this should move to internal module
// references and primordials throughout.
// deno-lint-ignore-file no-explicit-any prefer-primordials

(function () {
const { internals } = __bootstrap;
const {
  ArrayPrototypeIncludes,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  Error,
  JSONParse,
  JSONStringify,
  MathMin,
  Promise,
  SafeArrayIterator,
  StringPrototypeReplace,
  StringPrototypeStartsWith,
  TypeError,
  Uint8Array,
} = __bootstrap.primordials;

const LETS_ENCRYPT_DIRECTORY = "https://acme-v02.api.letsencrypt.org/directory";

// Maximum setTimeout delay; longer renewal deadlines are chained.
const MAX_TIMER_MS = 2 ** 31 - 1;

// Globals (fetch, crypto, Deno.*) are intentionally read lazily: this script
// is lazy-loaded on the first `Deno.serve` call, long after bootstrap.
function g() {
  return globalThis as any;
}

// ---------------------------------------------------------------------------
// bytes / base64 helpers
// ---------------------------------------------------------------------------

function concatBytes(chunks: Uint8Array[]): Uint8Array {
  let len = 0;
  for (const c of new SafeArrayIterator(chunks)) len += c.length;
  const out = new Uint8Array(len);
  let pos = 0;
  for (const c of new SafeArrayIterator(chunks)) {
    out.set(c, pos);
    pos += c.length;
  }
  return out;
}

function bytesToBase64(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i++) {
    bin += String.fromCharCode(bytes[i]);
  }
  return g().btoa(bin);
}

function base64ToBytes(b64: string): Uint8Array {
  const bin = g().atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) {
    out[i] = bin.charCodeAt(i);
  }
  return out;
}

function bytesToBase64Url(bytes: Uint8Array): string {
  return bytesToBase64(bytes)
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replace(/=+$/, "");
}

function utf8(s: string): Uint8Array {
  return new (g().TextEncoder)().encode(s);
}

function strToBase64Url(s: string): string {
  return bytesToBase64Url(utf8(s));
}

function pemToDer(pem: string): Uint8Array {
  const b64 = StringPrototypeReplace(pem, /-----[^-]+-----|\s+/g, "");
  return base64ToBytes(b64);
}

function derToPem(der: Uint8Array, label: string): string {
  const b64 = bytesToBase64(der);
  let out = `-----BEGIN ${label}-----\n`;
  for (let i = 0; i < b64.length; i += 64) {
    out += b64.slice(i, i + 64) + "\n";
  }
  out += `-----END ${label}-----\n`;
  return out;
}

// ---------------------------------------------------------------------------
// Minimal DER encoder (enough for a PKCS#10 CSR with a SAN extension)
// ---------------------------------------------------------------------------

function derTlv(tag: number, content: Uint8Array): Uint8Array {
  const len = content.length;
  let header;
  if (len < 0x80) {
    header = new Uint8Array([tag, len]);
  } else if (len < 0x100) {
    header = new Uint8Array([tag, 0x81, len]);
  } else if (len < 0x10000) {
    header = new Uint8Array([tag, 0x82, len >>> 8, len & 0xff]);
  } else {
    header = new Uint8Array([
      tag,
      0x83,
      len >>> 16,
      (len >>> 8) & 0xff,
      len & 0xff,
    ]);
  }
  return concatBytes([header, content]);
}

function derSeq(...parts: Uint8Array[]): Uint8Array {
  return derTlv(0x30, concatBytes(parts));
}

function derSet(...parts: Uint8Array[]): Uint8Array {
  return derTlv(0x31, concatBytes(parts));
}

// Context-specific constructed tag, eg. [0]
function derCtx(num: number, ...parts: Uint8Array[]): Uint8Array {
  return derTlv(0xa0 | num, concatBytes(parts));
}

function derOid(oid: string): Uint8Array {
  const parts = ArrayPrototypeMap(oid.split("."), Number);
  const bytes: number[] = [40 * parts[0] + parts[1]];
  for (let i = 2; i < parts.length; i++) {
    let n = parts[i];
    const enc = [n & 0x7f];
    n = Math.floor(n / 128);
    while (n > 0) {
      enc.unshift((n & 0x7f) | 0x80);
      n = Math.floor(n / 128);
    }
    for (const b of new SafeArrayIterator(enc)) ArrayPrototypePush(bytes, b);
  }
  return derTlv(0x06, new Uint8Array(bytes));
}

function derIntZero(): Uint8Array {
  return new Uint8Array([0x02, 0x01, 0x00]);
}

function derOctetString(content: Uint8Array): Uint8Array {
  return derTlv(0x04, content);
}

function derBitString(content: Uint8Array): Uint8Array {
  return derTlv(0x03, concatBytes([new Uint8Array([0x00]), content]));
}

// Unsigned big-endian integer to DER INTEGER (for ECDSA r/s values).
function derUint(bytes: Uint8Array): Uint8Array {
  let start = 0;
  while (start < bytes.length - 1 && bytes[start] === 0) start++;
  let trimmed = bytes.subarray(start);
  if (trimmed[0] & 0x80) {
    trimmed = concatBytes([new Uint8Array([0x00]), trimmed]);
  }
  return derTlv(0x02, trimmed);
}

// WebCrypto produces raw `r || s` ECDSA signatures; X.509/PKCS#10 wants the
// DER-encoded ECDSA-Sig-Value form.
function ecdsaRawSigToDer(raw: Uint8Array): Uint8Array {
  const half = raw.length / 2;
  return derSeq(derUint(raw.subarray(0, half)), derUint(raw.subarray(half)));
}

// ---------------------------------------------------------------------------
// Minimal DER reader (enough to extract certificate validity)
// ---------------------------------------------------------------------------

interface DerNode {
  tag: number;
  start: number;
  end: number;
}

function derReadNode(bytes: Uint8Array, pos: number): DerNode {
  const tag = bytes[pos];
  let len = bytes[pos + 1];
  let off = pos + 2;
  if (len & 0x80) {
    const n = len & 0x7f;
    len = 0;
    for (let i = 0; i < n; i++) {
      len = len * 256 + bytes[off++];
    }
  }
  return { tag, start: off, end: off + len };
}

function derChildren(bytes: Uint8Array, node: DerNode): DerNode[] {
  const out: DerNode[] = [];
  let pos = node.start;
  while (pos < node.end) {
    const child = derReadNode(bytes, pos);
    ArrayPrototypePush(out, child);
    pos = child.end;
  }
  return out;
}

function parseDerTime(bytes: Uint8Array, node: DerNode): number {
  let s = "";
  for (let i = node.start; i < node.end; i++) {
    s += String.fromCharCode(bytes[i]);
  }
  let year, rest;
  if (node.tag === 0x17) {
    // UTCTime: YYMMDDHHMMSSZ
    const yy = Number(s.slice(0, 2));
    year = yy >= 50 ? 1900 + yy : 2000 + yy;
    rest = s.slice(2);
  } else {
    // GeneralizedTime: YYYYMMDDHHMMSSZ
    year = Number(s.slice(0, 4));
    rest = s.slice(4);
  }
  const month = Number(rest.slice(0, 2));
  const day = Number(rest.slice(2, 4));
  const hour = Number(rest.slice(4, 6));
  const min = Number(rest.slice(6, 8));
  const sec = Number(rest.slice(8, 10));
  return Date.UTC(year, month - 1, day, hour, min, sec);
}

/** Extract `notBefore`/`notAfter` (ms since epoch) from the first
 * certificate in a PEM chain. */
function certValidity(
  certChainPem: string,
): { notBefore: number; notAfter: number } {
  const firstPem = certChainPem.match(
    /-----BEGIN CERTIFICATE-----[^-]+-----END CERTIFICATE-----/,
  );
  if (firstPem === null) {
    throw new Error("ACME: could not find certificate in PEM chain");
  }
  const der = pemToDer(firstPem[0]);
  const cert = derReadNode(der, 0);
  const tbs = derReadNode(der, cert.start);
  const fields = derChildren(der, tbs);
  // TBSCertificate: [0] version (optional), serialNumber, signature, issuer,
  // validity, ...
  let idx = 0;
  if (fields[0].tag === 0xa0) idx = 1;
  const validity = fields[idx + 3];
  const { 0: notBefore, 1: notAfter } = derChildren(der, validity);
  return {
    notBefore: parseDerTime(der, notBefore),
    notAfter: parseDerTime(der, notAfter),
  };
}

// ---------------------------------------------------------------------------
// CSR (PKCS#10) generation
// ---------------------------------------------------------------------------

async function createCsr(
  domains: string[],
  keyPair: { privateKey: unknown; publicKey: unknown },
): Promise<Uint8Array> {
  const subtle = g().crypto.subtle;
  const spki = new Uint8Array(
    await subtle.exportKey("spki", keyPair.publicKey),
  );
  // SubjectAltName extension: GeneralNames of dNSName ([2] IA5String)
  const generalNames = derSeq(
    ...ArrayPrototypeMap(domains, (d: string) => derTlv(0x82, utf8(d))),
  );
  const sanExtension = derSeq(
    derOid("2.5.29.17"),
    derOctetString(generalNames),
  );
  // attributes [0]: a single extensionRequest (PKCS#9) attribute
  const extensionRequest = derSeq(
    derOid("1.2.840.113549.1.9.14"),
    derSet(derSeq(sanExtension)),
  );
  const certificationRequestInfo = derSeq(
    derIntZero(), // version
    derSeq(), // empty subject; identity comes from SAN
    spki,
    derCtx(0, extensionRequest),
  );
  const rawSig = new Uint8Array(
    await subtle.sign(
      { name: "ECDSA", hash: "SHA-256" },
      keyPair.privateKey,
      certificationRequestInfo,
    ),
  );
  return derSeq(
    certificationRequestInfo,
    derSeq(derOid("1.2.840.10045.4.3.2")), // ecdsa-with-SHA256
    derBitString(ecdsaRawSigToDer(rawSig)),
  );
}

// ---------------------------------------------------------------------------
// ACME protocol client
// ---------------------------------------------------------------------------

interface AcmeDirectory {
  newNonce: string;
  newAccount: string;
  newOrder: string;
}

class AcmeClient {
  #directoryUrl: string;
  #directory: AcmeDirectory | null = null;
  #nonce: string | null = null;
  #accountKey: any = null;
  #publicJwk: any = null;
  accountUrl: string | null = null;
  thumbprint = "";

  constructor(directoryUrl: string) {
    this.#directoryUrl = directoryUrl;
  }

  async init(accountJwk: object | null, contact: string[]) {
    const subtle = g().crypto.subtle;
    if (accountJwk !== null) {
      this.#accountKey = await subtle.importKey(
        "jwk",
        accountJwk,
        { name: "ECDSA", namedCurve: "P-256" },
        true,
        ["sign"],
      );
      const { kty, crv, x, y } = accountJwk as any;
      this.#publicJwk = { kty, crv, x, y };
    } else {
      const pair = await subtle.generateKey(
        { name: "ECDSA", namedCurve: "P-256" },
        true,
        ["sign"],
      );
      this.#accountKey = pair.privateKey;
      const jwk = await subtle.exportKey("jwk", pair.publicKey);
      this.#publicJwk = { kty: jwk.kty, crv: jwk.crv, x: jwk.x, y: jwk.y };
    }
    // RFC 7638 JWK thumbprint: SHA-256 over the canonical public JWK.
    const canonical =
      `{"crv":"${this.#publicJwk.crv}","kty":"${this.#publicJwk.kty}","x":"${this.#publicJwk.x}","y":"${this.#publicJwk.y}"}`;
    const digest = new Uint8Array(
      await subtle.digest("SHA-256", utf8(canonical)),
    );
    this.thumbprint = bytesToBase64Url(digest);

    const dirResponse = await g().fetch(this.#directoryUrl);
    if (!dirResponse.ok) {
      throw new Error(
        `ACME: failed to fetch directory ${this.#directoryUrl}: HTTP ${dirResponse.status}`,
      );
    }
    this.#directory = await dirResponse.json();

    // Register (or look up) the account.
    const payload: any = { termsOfServiceAgreed: true };
    if (contact.length > 0) {
      payload.contact = ArrayPrototypeMap(
        contact,
        (c: string) =>
          StringPrototypeStartsWith(c, "mailto:") ? c : `mailto:${c}`,
      );
    }
    const res = await this.#signedFetch(
      this.#directory!.newAccount,
      payload,
      true,
    );
    await res.arrayBuffer(); // consume the account object
    this.accountUrl = res.headers.get("location");
    if (this.accountUrl === null) {
      throw new Error("ACME: newAccount response had no Location header");
    }
  }

  async exportAccountJwk(): Promise<object> {
    return await g().crypto.subtle.exportKey("jwk", this.#accountKey);
  }

  get directory(): AcmeDirectory {
    return this.#directory!;
  }

  async #getNonce(): Promise<string> {
    if (this.#nonce !== null) {
      const nonce = this.#nonce;
      this.#nonce = null;
      return nonce;
    }
    const res = await g().fetch(this.directory.newNonce, { method: "HEAD" });
    const nonce = res.headers.get("replay-nonce");
    if (nonce === null) {
      throw new Error("ACME: newNonce response had no Replay-Nonce header");
    }
    return nonce;
  }

  // POST a JWS-signed request. `payload` may be an object, or "" for
  // POST-as-GET. Uses the account URL (kid) unless `useJwk` is set.
  async #signedFetch(
    url: string,
    payload: object | "",
    useJwk = false,
    retried = false,
  ): Promise<any> {
    const subtle = g().crypto.subtle;
    const nonce = await this.#getNonce();
    const protectedHeader: any = { alg: "ES256", nonce, url };
    if (useJwk) {
      protectedHeader.jwk = this.#publicJwk;
    } else {
      protectedHeader.kid = this.accountUrl;
    }
    const protected64 = strToBase64Url(JSONStringify(protectedHeader));
    const payload64 = payload === "" ? "" : strToBase64Url(
      JSONStringify(payload),
    );
    const signature = new Uint8Array(
      await subtle.sign(
        { name: "ECDSA", hash: "SHA-256" },
        this.#accountKey,
        utf8(`${protected64}.${payload64}`),
      ),
    );
    const res = await g().fetch(url, {
      method: "POST",
      headers: { "content-type": "application/jose+json" },
      body: JSONStringify({
        protected: protected64,
        payload: payload64,
        signature: bytesToBase64Url(signature),
      }),
    });
    const newNonce = res.headers.get("replay-nonce");
    if (newNonce !== null) {
      this.#nonce = newNonce;
    }
    if (!res.ok) {
      let problem: any = {};
      try {
        problem = await res.json();
      } catch {
        // not a problem document
      }
      if (
        problem.type === "urn:ietf:params:acme:error:badNonce" && !retried
      ) {
        return await this.#signedFetch(url, payload, useJwk, true);
      }
      throw new Error(
        `ACME: request to ${url} failed (HTTP ${res.status}): ${
          problem.type ?? ""
        } ${problem.detail ?? ""}`,
      );
    }
    return res;
  }

  async post(url: string, payload: object): Promise<any> {
    const res = await this.#signedFetch(url, payload);
    return await res.json();
  }

  async postAsGetJson(url: string): Promise<any> {
    const res = await this.#signedFetch(url, "");
    return await res.json();
  }

  async postAsGetWithLocation(
    url: string,
    payload: object,
  ): Promise<{ body: any; location: string | null }> {
    const res = await this.#signedFetch(url, payload);
    return { body: await res.json(), location: res.headers.get("location") };
  }

  async postAsGetText(url: string): Promise<string> {
    const res = await this.#signedFetch(url, "");
    return await res.text();
  }
}

// ---------------------------------------------------------------------------
// HTTP-01 challenge server
// ---------------------------------------------------------------------------

const CHALLENGE_PATH_PREFIX = "/.well-known/acme-challenge/";

class ChallengeServer {
  #tokens: Map<string, string> = new Map();
  #server: any = null;
  #serveFn: any;
  #port: number;
  #hostname: string;

  constructor(serveFn: any, port: number, hostname: string) {
    this.#serveFn = serveFn;
    this.#port = port;
    this.#hostname = hostname;
  }

  addToken(token: string, keyAuthorization: string) {
    this.#tokens.set(token, keyAuthorization);
  }

  ensureStarted() {
    if (this.#server !== null) {
      return;
    }
    const tokens = this.#tokens;
    this.#server = this.#serveFn({
      port: this.#port,
      hostname: this.#hostname,
      onListen: () => {},
    }, (req: any) => {
      const Response = g().Response;
      const url = new (g().URL)(req.url);
      if (StringPrototypeStartsWith(url.pathname, CHALLENGE_PATH_PREFIX)) {
        const token = url.pathname.slice(CHALLENGE_PATH_PREFIX.length);
        const keyAuth = tokens.get(token);
        if (keyAuth !== undefined) {
          return new Response(keyAuth, {
            status: 200,
            headers: { "content-type": "text/plain" },
          });
        }
      }
      return new Response("not found", { status: 404 });
    });
  }

  async stop() {
    if (this.#server !== null) {
      const server = this.#server;
      this.#server = null;
      await server.shutdown();
    }
  }
}

// ---------------------------------------------------------------------------
// Certificate manager
// ---------------------------------------------------------------------------

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => g().setTimeout(resolve, ms));
}

async function pollUntil<T>(
  fn: () => Promise<T>,
  isDone: (v: T) => boolean,
  isFailed: (v: T) => boolean,
  what: string,
): Promise<T> {
  for (let attempt = 0; attempt < 60; attempt++) {
    const value = await fn();
    if (isDone(value)) {
      return value;
    }
    if (isFailed(value)) {
      throw new Error(
        `ACME: ${what} failed: ${JSONStringify(value)}`,
      );
    }
    await sleep(500);
  }
  throw new Error(`ACME: timed out waiting for ${what}`);
}

interface AcmeOptions {
  domains: string[];
  directoryUrl?: string;
  contact?: string | string[];
  cacheDir?: string;
  challengePort?: number;
  challengeHostname?: string;
}

interface CurrentCert {
  certChainPem: string;
  keyPem: string;
  notBefore: number;
  notAfter: number;
}

class AcmeCertManager {
  #domains: string[];
  #directoryUrl: string;
  #contact: string[];
  #cacheDir: string | null;
  #challengePort: number;
  #challengeHostname: string;
  #serveFn: any;
  #current: CurrentCert | null = null;
  #pending: Promise<CurrentCert> | null = null;
  #invalidate: ((sni: string) => void) | null = null;
  #renewTimer: number | null = null;
  #stopped = false;

  constructor(options: AcmeOptions, serveFn: any) {
    if (
      !Array.isArray(options.domains) || options.domains.length === 0
    ) {
      throw new TypeError(
        "ACME: `domains` must be a non-empty array of hostnames",
      );
    }
    this.#domains = options.domains;
    this.#directoryUrl = options.directoryUrl ?? LETS_ENCRYPT_DIRECTORY;
    this.#contact = typeof options.contact === "string"
      ? [options.contact]
      : (options.contact ?? []);
    this.#cacheDir = options.cacheDir ?? null;
    this.#challengePort = options.challengePort ?? 80;
    this.#challengeHostname = options.challengeHostname ?? "0.0.0.0";
    this.#serveFn = serveFn;
  }

  setInvalidator(invalidate: (sni: string) => void) {
    this.#invalidate = invalidate;
  }

  /** The SNI resolver callback for the TLS listener. */
  async resolveKeyPair(sni: string): Promise<{ cert: string; key: string }> {
    if (sni !== "" && !ArrayPrototypeIncludes(this.#domains, sni)) {
      throw new Error(
        `ACME: no certificate configured for hostname "${sni}"`,
      );
    }
    const current = await this.#ensure();
    return { cert: current.certChainPem, key: current.keyPem };
  }

  /** Eagerly provision (or load) the certificate and schedule renewal. */
  start() {
    this.#ensure().then(() => {
      this.#scheduleRenewal();
    }, (err) => {
      internals.log(
        "error",
        `ACME: failed to provision certificate for [${
          ArrayPrototypeJoin(this.#domains, ", ")
        }]:`,
        err,
      );
    });
  }

  stop() {
    this.#stopped = true;
    if (this.#renewTimer !== null) {
      g().clearTimeout(this.#renewTimer);
      this.#renewTimer = null;
    }
  }

  #needsRenewal(cert: CurrentCert): boolean {
    const lifetime = cert.notAfter - cert.notBefore;
    return Date.now() > cert.notAfter - lifetime / 3;
  }

  async #ensure(): Promise<CurrentCert> {
    if (this.#current !== null && !this.#needsRenewal(this.#current)) {
      return this.#current;
    }
    if (this.#pending === null) {
      this.#pending = (async () => {
        try {
          // Try the disk cache first (only when no cert is loaded yet).
          if (this.#current === null) {
            const cached = await this.#loadFromCache();
            if (cached !== null && !this.#needsRenewal(cached)) {
              this.#current = cached;
              return cached;
            }
          }
          const fresh = await this.#provision();
          const hadCert = this.#current !== null;
          this.#current = fresh;
          await this.#saveToCache(fresh);
          if (hadCert && this.#invalidate !== null) {
            // Drop cached resolutions so new handshakes pick up the new cert.
            this.#invalidate("");
            for (const domain of new SafeArrayIterator(this.#domains)) {
              this.#invalidate(domain);
            }
          }
          return fresh;
        } finally {
          this.#pending = null;
        }
      })();
    }
    return await this.#pending;
  }

  #scheduleRenewal() {
    if (this.#stopped || this.#current === null) {
      return;
    }
    const cert = this.#current;
    const lifetime = cert.notAfter - cert.notBefore;
    const renewAt = cert.notAfter - lifetime / 3;
    const delay = MathMin(Math.max(renewAt - Date.now(), 1000), MAX_TIMER_MS);
    this.#renewTimer = g().setTimeout(async () => {
      this.#renewTimer = null;
      if (this.#stopped) {
        return;
      }
      try {
        await this.#ensure();
      } catch (err) {
        internals.log("error", "ACME: certificate renewal failed:", err);
      }
      this.#scheduleRenewal();
    }, delay);
    g().Deno.unrefTimer(this.#renewTimer);
  }

  // -- disk cache -----------------------------------------------------------

  #cachePathBase(): string | null {
    if (this.#cacheDir === null) {
      return null;
    }
    const name = StringPrototypeReplace(
      ArrayPrototypeJoin(this.#domains, "_"),
      /[^a-zA-Z0-9._-]/g,
      "_",
    );
    return `${this.#cacheDir}/${name}`;
  }

  async #loadFromCache(): Promise<CurrentCert | null> {
    const base = this.#cachePathBase();
    if (base === null) {
      return null;
    }
    const Deno = g().Deno;
    try {
      const certChainPem = await Deno.readTextFile(`${base}.crt.pem`);
      const keyPem = await Deno.readTextFile(`${base}.key.pem`);
      const { notBefore, notAfter } = certValidity(certChainPem);
      return { certChainPem, keyPem, notBefore, notAfter };
    } catch {
      return null;
    }
  }

  async #saveToCache(cert: CurrentCert) {
    const base = this.#cachePathBase();
    if (base === null) {
      return;
    }
    const Deno = g().Deno;
    try {
      await Deno.mkdir(this.#cacheDir, { recursive: true });
      await Deno.writeTextFile(`${base}.crt.pem`, cert.certChainPem);
      await Deno.writeTextFile(`${base}.key.pem`, cert.keyPem, {
        mode: 0o600,
      });
    } catch (err) {
      internals.log("error", "ACME: failed to write certificate cache:", err);
    }
  }

  async #loadAccountJwk(): Promise<object | null> {
    if (this.#cacheDir === null) {
      return null;
    }
    try {
      return JSONParse(
        await g().Deno.readTextFile(`${this.#cacheDir}/account_key.json`),
      );
    } catch {
      return null;
    }
  }

  async #saveAccountJwk(jwk: object) {
    if (this.#cacheDir === null) {
      return;
    }
    const Deno = g().Deno;
    try {
      await Deno.mkdir(this.#cacheDir, { recursive: true });
      await Deno.writeTextFile(
        `${this.#cacheDir}/account_key.json`,
        JSONStringify(jwk),
        { mode: 0o600 },
      );
    } catch (err) {
      internals.log("error", "ACME: failed to write account key:", err);
    }
  }

  // -- provisioning ---------------------------------------------------------

  async #provision(): Promise<CurrentCert> {
    const subtle = g().crypto.subtle;
    const client = new AcmeClient(this.#directoryUrl);
    const cachedJwk = await this.#loadAccountJwk();
    await client.init(cachedJwk, this.#contact);
    if (cachedJwk === null) {
      await this.#saveAccountJwk(await client.exportAccountJwk());
    }

    const { body: order, location: orderUrl } = await client
      .postAsGetWithLocation(client.directory.newOrder, {
        identifiers: ArrayPrototypeMap(
          this.#domains,
          (d: string) => ({ type: "dns", value: d }),
        ),
      });
    if (orderUrl === null) {
      throw new Error("ACME: newOrder response had no Location header");
    }

    const challengeServer = new ChallengeServer(
      this.#serveFn,
      this.#challengePort,
      this.#challengeHostname,
    );
    try {
      for (
        const authzUrl of new SafeArrayIterator(order.authorizations ?? [])
      ) {
        const authz = await client.postAsGetJson(authzUrl);
        if (authz.status === "valid") {
          continue;
        }
        const challenge = (authz.challenges ?? []).find(
          (c: any) => c.type === "http-01",
        );
        if (challenge === undefined) {
          throw new Error(
            `ACME: no http-01 challenge offered for ${authz.identifier?.value}`,
          );
        }
        const keyAuthorization = `${challenge.token}.${client.thumbprint}`;
        challengeServer.addToken(challenge.token, keyAuthorization);
        challengeServer.ensureStarted();
        await client.post(challenge.url, {});
        await pollUntil(
          () => client.postAsGetJson(authzUrl),
          (a: any) => a.status === "valid",
          (a: any) => a.status === "invalid" || a.status === "revoked",
          `authorization for ${authz.identifier?.value}`,
        );
      }

      const certKeyPair = await subtle.generateKey(
        { name: "ECDSA", namedCurve: "P-256" },
        true,
        ["sign"],
      );
      const csr = await createCsr(this.#domains, certKeyPair);
      await client.post(order.finalize, { csr: bytesToBase64Url(csr) });
      const finalOrder = await pollUntil(
        () => client.postAsGetJson(orderUrl),
        (o: any) => o.status === "valid" && o.certificate !== undefined,
        (o: any) => o.status === "invalid",
        "order finalization",
      );

      const certChainPem = await client.postAsGetText(finalOrder.certificate);
      const pkcs8 = new Uint8Array(
        await subtle.exportKey("pkcs8", certKeyPair.privateKey),
      );
      const keyPem = derToPem(pkcs8, "PRIVATE KEY");
      const { notBefore, notAfter } = certValidity(certChainPem);
      internals.log(
        "info",
        `ACME: obtained certificate for [${
          ArrayPrototypeJoin(this.#domains, ", ")
        }] (expires ${new Date(notAfter).toISOString()})`,
      );
      return { certChainPem, keyPem, notBefore, notAfter };
    } finally {
      await challengeServer.stop();
    }
  }
}

function createAcmeCertManager(options: AcmeOptions, serveFn: any) {
  return new AcmeCertManager(options, serveFn);
}

return { createAcmeCertManager };
})();
