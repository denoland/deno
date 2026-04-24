// Copyright 2018-2026 the Deno authors. MIT license.

// Remote backend for Deno KV using the KV Connect protocol.
// Communicates with Deno Deploy (or any compatible endpoint) over HTTP,
// using hand-rolled protobuf from the sibling module for the data path
// and JSON for metadata exchange.

import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeMap,
  Date,
  DateNow,
  DatePrototypeGetTime,
  Error,
  JSONStringify,
  MathMax,
  MathRandom,
  Promise,
  PromisePrototypeCatch,
  String,
  TypedArrayPrototypeGetLength,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeSlice,
  Uint8Array,
} = primordials;

import {
  type AtomicWriteOutput,
  AtomicWriteStatus,
  type Check,
  decodeAtomicWriteOutput,
  decodeSnapshotReadOutput,
  decodeWatchOutput,
  encodeAtomicWrite,
  encodeSnapshotRead,
  encodeWatch,
  type Enqueue,
  type KvEntry as ProtoKvEntry,
  type Mutation,
  type ReadRange,
  type SnapshotReadOutput,
  SnapshotReadStatus,
  type WatchOutput,
} from "./protobuf.ts";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export interface RemoteKvEntry {
  key: Uint8Array;
  value: Uint8Array;
  encoding: number;
  versionstamp: Uint8Array;
}

export interface WatchKeyUpdate {
  changed: boolean;
  entry?: RemoteKvEntry;
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

interface MetadataEndpoint {
  url: string;
  consistency: string;
}

interface Metadata {
  version: number;
  databaseId: string;
  endpoints: MetadataEndpoint[];
  token: string;
  expiresAt: Date;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SUPPORTED_VERSIONS = [1, 2, 3];

/** Refresh metadata this many ms before the token expires. */
const REFRESH_BEFORE_EXPIRY_MS = 10 * 60 * 1000; // 10 minutes

/** Minimum refresh interval when the expiry is very close. */
const MIN_REFRESH_INTERVAL_MS = 60 * 1000; // 1 minute

/** Base delay for metadata fetch retry backoff. */
const METADATA_RETRY_BASE_MS = 5_000;

/** Base delay for data-path retry backoff (5xx / network errors). */
const DATA_RETRY_BASE_MS = 200;

/** Maximum number of data-path retries. */
const MAX_DATA_RETRIES = 12;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Exponential backoff with jitter. */
function backoffDelay(base: number, attempt: number): number {
  const exponential = base * (2 ** attempt);
  const jitter = MathRandom() * base;
  return exponential + jitter;
}

function protoEntryToRemote(e: ProtoKvEntry): RemoteKvEntry {
  return {
    key: e.key,
    value: e.value,
    encoding: e.encoding,
    versionstamp: e.versionstamp,
  };
}

// ---------------------------------------------------------------------------
// RemoteBackend
// ---------------------------------------------------------------------------

export class RemoteBackend {
  #metadataUrl: string;
  #accessToken: string;
  #metadata: Metadata | null = null;
  #metadataRefreshTimer: number | null = null;
  #closed = false;

  constructor(metadataUrl: string, accessToken: string) {
    this.#metadataUrl = metadataUrl;
    this.#accessToken = accessToken;
  }

  // -----------------------------------------------------------------------
  // Public API
  // -----------------------------------------------------------------------

  async snapshotRead(
    ranges: ReadRange[],
    consistency: "strong" | "eventual",
  ): Promise<RemoteKvEntry[][]> {
    const body = encodeSnapshotRead(ranges);
    const respBytes = await this.#post("snapshot_read", body, consistency);
    const output: SnapshotReadOutput = decodeSnapshotReadOutput(respBytes);

    if (output.status === SnapshotReadStatus.SR_READ_DISABLED) {
      throw new Error("KV read operations are disabled for this database");
    }
    if (
      output.status !== SnapshotReadStatus.SR_SUCCESS &&
      output.status !== SnapshotReadStatus.SR_UNSPECIFIED
    ) {
      throw new Error(
        `Snapshot read failed with status ${output.status}`,
      );
    }

    return ArrayPrototypeMap(
      output.ranges,
      (range) => ArrayPrototypeMap(range.values, protoEntryToRemote),
    );
  }

  async atomicWrite(
    checks: Check[],
    mutations: Mutation[],
    enqueues: Enqueue[],
  ): Promise<{ versionstamp: Uint8Array } | null> {
    const body = encodeAtomicWrite({ checks, mutations, enqueues });
    const respBytes = await this.#post("atomic_write", body, "strong");
    const output: AtomicWriteOutput = decodeAtomicWriteOutput(respBytes);

    if (output.status === AtomicWriteStatus.AW_CHECK_FAILURE) {
      return null;
    }
    if (output.status === AtomicWriteStatus.AW_WRITE_DISABLED) {
      throw new Error("KV write operations are disabled for this database");
    }
    if (
      output.status !== AtomicWriteStatus.AW_SUCCESS &&
      output.status !== AtomicWriteStatus.AW_UNSPECIFIED
    ) {
      throw new Error(
        `Atomic write failed with status ${output.status}`,
      );
    }

    return { versionstamp: output.versionstamp };
  }

  watch(keys: Uint8Array[]): ReadableStream<WatchKeyUpdate[]> {
    let reader: ReadableStreamDefaultReader<Uint8Array> | null = null;
    let abortController: AbortController | null = null;
    let buffer = new Uint8Array(0);
    let reconnectAttempt = 0;

    // deno-lint-ignore no-this-alias
    const self = this;

    return new ReadableStream<WatchKeyUpdate[]>({
      pull: async (
        controller: ReadableStreamDefaultController<WatchKeyUpdate[]>,
      ) => {
        // Outer loop: reconnect on stream errors
        while (true) {
          try {
            // Establish connection if we don't have one
            if (reader === null) {
              const result = await self.#startWatchStream(keys);
              reader = result.reader;
              abortController = result.abortController;
              buffer = new Uint8Array(0);
              reconnectAttempt = 0;
            }

            // Inner loop: read frames from the current connection
            while (true) {
              // Ensure we have at least 4 bytes for the frame length
              while (TypedArrayPrototypeGetLength(buffer) < 4) {
                const { done, value } = await reader.read();
                if (done) {
                  // Stream ended - force reconnect
                  reader = null;
                  abortController = null;
                  break;
                }
                buffer = appendBuffer(buffer, value);
              }

              if (reader === null) break; // reconnect

              // Read the 4-byte little-endian frame length
              const frameLen = buffer[0] |
                (buffer[1] << 8) |
                (buffer[2] << 16) |
                (buffer[3] << 24);
              // Use unsigned interpretation
              const frameLenU = frameLen >>> 0;

              // Ensure we have the full frame
              while (TypedArrayPrototypeGetLength(buffer) < 4 + frameLenU) {
                const { done, value } = await reader.read();
                if (done) {
                  throw new Error(
                    "Watch stream ended mid-frame",
                  );
                }
                buffer = appendBuffer(buffer, value);
              }

              // Extract the frame and advance the buffer
              const frame = TypedArrayPrototypeSlice(buffer, 4, 4 + frameLenU);
              buffer = TypedArrayPrototypeSlice(buffer, 4 + frameLenU);

              // Empty frames are pings - skip them
              if (frameLenU === 0) {
                continue;
              }

              const output: WatchOutput = decodeWatchOutput(frame);

              if (output.status === SnapshotReadStatus.SR_READ_DISABLED) {
                controller.error(
                  new Error(
                    "KV read operations are disabled for this database",
                  ),
                );
                return;
              }

              const updates: WatchKeyUpdate[] = ArrayPrototypeMap(
                output.keys,
                (k) => {
                  const update: WatchKeyUpdate = { changed: k.changed };
                  if (k.entryIfChanged !== null) {
                    update.entry = protoEntryToRemote(k.entryIfChanged);
                  }
                  return update;
                },
              );

              controller.enqueue(updates);
              return; // One frame per pull
            }
          } catch (err) {
            // Clean up the failed stream
            reader = null;
            if (abortController !== null) {
              try {
                abortController.abort();
              } catch {
                // ignore
              }
              abortController = null;
            }

            if (self.#closed) {
              controller.close();
              return;
            }

            // Exponential backoff before reconnecting
            const delayMs = backoffDelay(DATA_RETRY_BASE_MS, reconnectAttempt);
            reconnectAttempt++;

            if (reconnectAttempt > MAX_DATA_RETRIES) {
              controller.error(
                new Error(
                  `Watch stream failed after ${MAX_DATA_RETRIES} reconnect attempts: ${err}`,
                ),
              );
              return;
            }

            await delay(delayMs);
            // Loop back to reconnect
          }
        }
      },

      cancel: () => {
        if (abortController !== null) {
          abortController.abort();
          abortController = null;
        }
        reader = null;
      },
    });
  }

  close(): void {
    this.#closed = true;
    if (this.#metadataRefreshTimer !== null) {
      clearTimeout(this.#metadataRefreshTimer);
      this.#metadataRefreshTimer = null;
    }
  }

  // -----------------------------------------------------------------------
  // Metadata management
  // -----------------------------------------------------------------------

  async #ensureMetadata(): Promise<Metadata> {
    if (
      this.#metadata !== null &&
      DatePrototypeGetTime(this.#metadata.expiresAt) > DateNow()
    ) {
      return this.#metadata;
    }
    return await this.#fetchMetadata();
  }

  async #fetchMetadata(): Promise<Metadata> {
    let attempt = 0;
    while (true) {
      try {
        const resp = await fetch(this.#metadataUrl, {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "authorization": `Bearer ${this.#accessToken}`,
          },
          body: JSONStringify({ supportedVersions: SUPPORTED_VERSIONS }),
        });

        if (!resp.ok) {
          const text = await PromisePrototypeCatch(
            resp.text(),
            () => "",
          );
          throw new Error(
            `Metadata exchange failed (HTTP ${resp.status}): ${text}`,
          );
        }

        const json = await resp.json();
        const metadata: Metadata = {
          version: json.version,
          databaseId: json.databaseId,
          endpoints: json.endpoints,
          token: json.token,
          expiresAt: new Date(json.expiresAt),
        };

        this.#metadata = metadata;
        this.#scheduleMetadataRefresh(metadata);
        return metadata;
      } catch (err) {
        attempt++;
        const delayMs = backoffDelay(METADATA_RETRY_BASE_MS, attempt);
        if (attempt > 5) {
          throw new Error(
            `Failed to fetch KV metadata after ${attempt} attempts: ${err}`,
          );
        }
        await delay(delayMs);
      }
    }
  }

  #scheduleMetadataRefresh(metadata: Metadata): void {
    if (this.#metadataRefreshTimer !== null) {
      clearTimeout(this.#metadataRefreshTimer);
    }

    const timeUntilExpiry = DatePrototypeGetTime(metadata.expiresAt) -
      DateNow();
    const refreshIn = MathMax(
      MIN_REFRESH_INTERVAL_MS,
      timeUntilExpiry - REFRESH_BEFORE_EXPIRY_MS,
    );

    this.#metadataRefreshTimer = setTimeout(() => {
      if (!this.#closed) {
        PromisePrototypeCatch(this.#fetchMetadata(), () => {
          // Refresh failure is non-fatal; the next operation will retry.
        });
      }
    }, refreshIn) as unknown as number;

    // Don't let the refresh timer keep the process alive
    if (
      typeof this.#metadataRefreshTimer === "number" &&
      typeof Deno !== "undefined"
    ) {
      Deno.unrefTimer(this.#metadataRefreshTimer);
    }
  }

  // -----------------------------------------------------------------------
  // Data path
  // -----------------------------------------------------------------------

  async #post(
    method: string,
    body: Uint8Array,
    consistency?: "strong" | "eventual",
  ): Promise<Uint8Array> {
    const metadata = await this.#ensureMetadata();
    const endpoint = this.#pickEndpoint(metadata, consistency ?? "strong");
    const url = `${endpoint.url}/${method}`;
    const headers = this.#buildHeaders(metadata);

    let attempt = 0;
    while (true) {
      let resp: Response;
      try {
        resp = await fetch(url, {
          method: "POST",
          headers,
          body,
        });
      } catch (err) {
        // Network error - treat like a 5xx
        if (attempt >= MAX_DATA_RETRIES) {
          throw new Error(
            `KV ${method} request failed after ${attempt} retries: ${err}`,
          );
        }
        await delay(backoffDelay(DATA_RETRY_BASE_MS, attempt));
        attempt++;
        continue;
      }

      if (resp.status >= 500 && resp.status < 600) {
        if (attempt >= MAX_DATA_RETRIES) {
          const text = await PromisePrototypeCatch(
            resp.text(),
            () => "",
          );
          throw new Error(
            `KV ${method} request failed with HTTP ${resp.status} after ${attempt} retries: ${text}`,
          );
        }
        await delay(backoffDelay(DATA_RETRY_BASE_MS, attempt));
        attempt++;
        continue;
      }

      if (resp.status >= 400 && resp.status < 500) {
        const text = await PromisePrototypeCatch(
          resp.text(),
          () => "",
        );
        if (resp.status === 401 || resp.status === 403) {
          throw new Error(
            `KV authentication error (HTTP ${resp.status}): ${text}`,
          );
        }
        throw new Error(
          `KV ${method} request failed with HTTP ${resp.status}: ${text}`,
        );
      }

      const arrayBuffer = await resp.arrayBuffer();
      return new Uint8Array(arrayBuffer);
    }
  }

  async #startWatchStream(keys: Uint8Array[]): Promise<{
    reader: ReadableStreamDefaultReader<Uint8Array>;
    abortController: AbortController;
  }> {
    const metadata = await this.#ensureMetadata();
    const endpoint = this.#pickEndpoint(metadata, "strong");
    const url = `${endpoint.url}/watch`;
    const headers = this.#buildHeaders(metadata);
    const body = encodeWatch(keys);

    const abortController = new AbortController();

    const resp = await fetch(url, {
      method: "POST",
      headers,
      body,
      signal: abortController.signal,
    });

    if (!resp.ok) {
      const text = await PromisePrototypeCatch(
        resp.text(),
        () => "",
      );
      throw new Error(
        `Watch request failed with HTTP ${resp.status}: ${text}`,
      );
    }

    if (resp.body === null) {
      throw new Error("Watch response has no body");
    }

    const reader = resp.body.getReader();
    return { reader, abortController };
  }

  // -----------------------------------------------------------------------
  // Header building
  // -----------------------------------------------------------------------

  #buildHeaders(metadata: Metadata): Record<string, string> {
    const headers: Record<string, string> = {
      "authorization": `Bearer ${metadata.token}`,
      "content-type": "application/x-protobuf",
    };

    switch (metadata.version) {
      case 1:
        headers["x-transaction-domain-id"] = metadata.databaseId;
        break;
      case 2:
        headers["x-denokv-database-id"] = metadata.databaseId;
        headers["x-denokv-version"] = "2";
        break;
      case 3:
      default:
        headers["x-denokv-database-id"] = metadata.databaseId;
        headers["x-denokv-version"] = String(metadata.version);
        break;
    }

    return headers;
  }

  // -----------------------------------------------------------------------
  // Endpoint selection
  // -----------------------------------------------------------------------

  #pickEndpoint(
    metadata: Metadata,
    consistency: "strong" | "eventual",
  ): MetadataEndpoint {
    const endpoints = metadata.endpoints;
    const endpointsLen = endpoints.length;

    // Try to find an endpoint matching the requested consistency
    for (let i = 0; i < endpointsLen; i++) {
      const ep = endpoints[i];
      if (ep.consistency === consistency) {
        return ep;
      }
    }

    // Fall back to "strong" if "eventual" was requested but not available,
    // since strong is always acceptable.
    if (consistency === "eventual") {
      for (let i = 0; i < endpointsLen; i++) {
        const ep = endpoints[i];
        if (ep.consistency === "strong") {
          return ep;
        }
      }
    }

    // Last resort: use the first endpoint
    if (endpointsLen > 0) {
      return endpoints[0];
    }

    throw new Error("No KV endpoints available in metadata");
  }
}

// ---------------------------------------------------------------------------
// Buffer utilities
// ---------------------------------------------------------------------------

function appendBuffer(existing: Uint8Array, chunk: Uint8Array): Uint8Array {
  const existingLen = TypedArrayPrototypeGetLength(existing);
  const chunkLen = TypedArrayPrototypeGetLength(chunk);
  const combined = new Uint8Array(existingLen + chunkLen);
  TypedArrayPrototypeSet(combined, existing, 0);
  TypedArrayPrototypeSet(combined, chunk, existingLen);
  return combined;
}
