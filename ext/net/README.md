# deno_net

**This crate implements networking APIs.**

## Usage Example

From javascript, include the extension's source:

```javascript
import * as webidl from "ext:deno_webidl/00_webidl.js";
import * as net from "ext:deno_net/01_net.js";
import * as tls from "ext:deno_net/02_tls.js";
```

Then from rust, provide:
`deno_net::deno_net::init_ops_and_esm::<Permissions>(root_cert_store_provider, unsafely_ignore_certificate_errors)`

Where:

- root_cert_store_provider: `Option<Arc<dyn RootCertStoreProvider>>`
- unsafely_ignore_certificate_errors: `Option<Vec<String>>`
- Permissions: A struct implementing `deno_net::NetPermissions`

In the `extensions` field of your `RuntimeOptions`

## Dependencies

- **deno_web**: Provided by the `deno_web` crate
- **deno_fetch**: Provided by the `deno_fetch` crate

## Provided ops

Following ops are provided, which can be accessed through `Deno.ops`:

### Net

- op_net_accept_tcp
- op_net_accept_unix
- op_net_connect_tcp
- op_net_connect_unix
- op_net_listen_tcp
- op_net_listen_udp
- op_net_listen_unix
- op_net_listen_unixpacket
- op_net_recv_udp
- op_net_recv_unixpacket
- op_net_send_udp
- op_net_send_unixpacket
- op_net_connect_tls
- op_net_listen_tls
- op_net_accept_tls
- op_net_recv_udp
- op_net_send_udp
- op_net_join_multi_v4_udp
- op_net_join_multi_v6_udp
- op_net_leave_multi_v4_udp
- op_net_leave_multi_v6_udp
- op_net_set_multi_loopback_udp
- op_net_set_multi_ttl_udp
- op_net_accept_tcp
- op_net_connect_tcp
- op_net_listen_tcp
- op_net_listen_udp
- op_net_connect_tls
- op_net_listen_tls
- op_net_accept_tls
- op_net_accept_unix
- op_net_connect_unix
- op_net_listen_unix
- op_net_listen_unixpacket
- op_net_recv_unixpacket
- op_net_send_unixpacket

### TLS

- op_tls_start
- op_tls_handshake
- op_tls_key_null
- op_tls_key_static
- op_tls_key_static_from_file
- op_tls_cert_resolver_create
- op_tls_cert_resolver_poll
- op_tls_cert_resolver_resolve
- op_tls_cert_resolver_resolve_error
- op_tls_start
- op_tls_handshake

### Other

- op_node_unstable_net_listen_udp
- op_dns_resolve
- op_dns_resolve
- op_set_nodelay
- op_set_keepalive
- op_node_unstable_net_listen_unixpacket
