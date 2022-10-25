# deno_net

This crate implements networking APIs.

This crate depends on following extensions:

- "deno_web"
- "deno_fetch"

Following ops are provided:

- "op_net_accept_tcp"
- "op_net_accept_unix"
- "op_net_connect_tcp"
- "op_net_connect_unix"
- "op_net_listen_tcp"
- "op_net_listen_udp"
- "op_net_listen_unix"
- "op_net_listen_unixpacket"
- "op_net_recv_udp"
- "op_net_recv_unixpacket"
- "op_net_send_udp"
- "op_net_send_unixpacket"
- "op_dns_resolve"
- "op_net_connect_tls"
- "op_net_listen_tls"
- "op_net_accept_tls"
- "op_tls_start"
- "op_tls_handshake"
