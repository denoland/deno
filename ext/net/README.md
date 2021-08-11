# deno_net

This crate implements networking APIs.

This crate depends on following extensions:

- "deno_web"
- "deno_fetch"

Following ops are provided:

- "op_net_read_async"
- "op_net_write_async"
- "op_net_shutdown"
- "op_accept"
- "op_connect"
- "op_listen"
- "op_datagram_receive"
- "op_datagram_send"
- "op_dns_resolve"
- "op_start_tls"
- "op_connect_tls"
- "op_listen_tls"
- "op_accept_tls"
- "op_http_start"
- "op_http_request_next"
- "op_http_request_read"
- "op_http_response"
- "op_http_response_write"
- "op_http_response_close"
