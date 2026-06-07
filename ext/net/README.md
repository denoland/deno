# deno_net

**This crate implements networking APIs.**

## Usage Example

From javascript, include the extension's source:

```javascript
import { core } from "ext:core/mod.js";

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const net = core.loadExtScript("ext:deno_net/01_net.js");
const tls = core.loadExtScript("ext:deno_net/02_tls.js");
const loadQuic = core.createLazyLoader("ext:deno_net/03_quic.js");
const quic = loadQuic();
```

Then from rust, provide:
`deno_net::deno_net::init(root_cert_store_provider, unsafely_ignore_certificate_errors)`

Where:

- root_cert_store_provider: `Option<Arc<dyn RootCertStoreProvider>>`
- unsafely_ignore_certificate_errors: `Option<Vec<String>>`

In the `extensions` field of your `RuntimeOptions`

## Dependencies

- **deno_web**: Provided by the `deno_web` crate
- **deno_fetch**: Provided by the `deno_fetch` crate

## Provided ops

Following ops are provided, which can be accessed through `Deno.ops`:

### Net

- op_net_accept_tcp
- op_net_get_ips_from_perm_token
- op_net_connect_tcp
- op_net_listen_tcp
- op_net_listen_udp
- op_net_recv_udp
- op_net_send_udp
- op_net_join_multi_v4_udp
- op_net_join_multi_v6_udp
- op_net_leave_multi_v4_udp
- op_net_leave_multi_v6_udp
- op_net_set_multi_loopback_udp
- op_net_set_multi_ttl_udp
- op_net_set_broadcast_udp
- op_net_validate_multicast
- op_net_get_system_dns_servers
- op_net_listen_vsock
- op_net_accept_vsock
- op_net_connect_vsock
- op_net_listen_tunnel
- op_net_accept_tunnel
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

- op_tls_key_null
- op_tls_key_static
- op_tls_cert_resolver_create
- op_tls_cert_resolver_poll
- op_tls_cert_resolver_resolve
- op_tls_cert_resolver_resolve_error
- op_tls_start
- op_tls_handshake

### QUIC

- op_quic_connecting_0rtt
- op_quic_connecting_1rtt
- op_quic_connection_accept_bi
- op_quic_connection_accept_uni
- op_quic_connection_close
- op_quic_connection_closed
- op_quic_connection_get_protocol
- op_quic_connection_get_remote_addr
- op_quic_connection_get_server_name
- op_quic_connection_handshake
- op_quic_connection_open_bi
- op_quic_connection_open_uni
- op_quic_connection_get_max_datagram_size
- op_quic_connection_read_datagram
- op_quic_connection_send_datagram
- op_quic_endpoint_close
- op_quic_endpoint_connect
- op_quic_endpoint_create
- op_quic_endpoint_get_addr
- op_quic_endpoint_listen
- op_quic_incoming_accept
- op_quic_incoming_accept_0rtt
- op_quic_incoming_ignore
- op_quic_incoming_local_ip
- op_quic_incoming_refuse
- op_quic_incoming_remote_addr
- op_quic_incoming_remote_addr_validated
- op_quic_listener_accept
- op_quic_listener_stop
- op_quic_recv_stream_get_id
- op_quic_send_stream_get_id
- op_quic_send_stream_get_priority
- op_quic_send_stream_set_priority

### WebTransport

- op_webtransport_accept
- op_webtransport_connect

### Other

- op_node_unstable_net_listen_udp
- op_dns_resolve
- op_set_nodelay
- op_set_keepalive
- op_node_unstable_net_listen_unixpacket
