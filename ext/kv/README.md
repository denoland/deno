# deno_kv

This crate provides a key/value store for Deno. For an overview of Deno KV,
please read the [manual](https://deno.land/manual/runtime/kv).

## Storage Backends

Deno KV has a pluggable storage interface that supports multiple backends:

- SQLite - backed by a local SQLite database. This backend is suitable for
  development and is the default when running locally.
- Remote - backed by a remote service that implements the
  [KV Connect](#kv-connect) protocol, for example
  [Deno Deploy](https://deno.com/deploy).

Additional backends can be added by implementing the `DatabaseHandler` trait.

## KV Connect

The KV Connect protocol has separate control and data planes to maximize
throughput and minimize latency. _Metadata Exchange_ and _Data Path_ are the two
sub-protocols that are used when talking to a KV Connect-compatible service.

### Metadata Exchange

To connect to a KV Connect service, the user provides an HTTP or HTTPS URL to
`Deno.openKv`. A background task is then spawned to periodically make HTTP POST
requests to the provided URL to refresh database metadata.

The HTTP `Authorization` header is included and have the format
`Bearer <access-token>`. The `<access-token>` is a static token issued by the
service provider. For Deno Deploy, this is the personal access token generated
from the dashboard. You can specify the access token with the environment
variable `DENO_KV_ACCESS_TOKEN`.

Request body is currently unused. The response is a JSON message that satisfies
the [JSON Schema](https://json-schema.org/) definition in
`cli/schemas/kv-metadata-exchange-response.v1.json`.

Semantics of the response fields:

- `version`: Protocol version. The only supported value is `1`.
- `databaseId`: UUID of the database.
- `endpoints`: Data plane endpoints that can serve requests to the database,
  along with their consistency levels.
- `token`: An ephemeral authentication token that must be included in all
  requests to the data plane. This value is an opaque string and the client
  should not depend on its format.
- `expiresAt`: The time at which the token expires. Encoded as an ISO 8601
  string.

### Data Path

After the first metadata exchange has completed, the client can talk to the data
plane endpoints listed in the `endpoints` field using a Protobuf-over-HTTP
protocol called the _Data Path_. The Protobuf messages are defined in
`proto/datapath.proto`.

Two sub-endpoints are available under a data plane endpoint URL:

- `POST /snapshot_read`: Used for read operations: `kv.get()` and
  `kv.getMany()`.
  - **Request type**: `SnapshotRead`
  - **Response type**: `SnapshotReadOutput`
- `POST /atomic_write`: Used for write operations: `kv.set()` and
  `kv.atomic().commit()`.
  - **Request type**: `AtomicWrite`
  - **Response type**: `AtomicWriteOutput`

An HTTP `Authorization` header in the format `Bearer <ephemeral-token>` must be
included in all requests to the data plane. The value of `<ephemeral-token>` is
the `token` field from the metadata exchange response.

### Error handling

All non-client errors (i.e. network errors and HTTP 5xx status codes) are
handled by retrying the request. Randomized exponential backoff is applied to
each retry.

Client errors cannot be recovered by retrying. A JavaScript exception is
generated for each of those errors.
