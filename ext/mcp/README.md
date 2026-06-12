# deno_mcp

This crate implements the unstable `Deno.McpServer` API, which makes it easy to
write Model Context Protocol (MCP) servers in Deno.

It implements the MCP JSON-RPC lifecycle (initialize handshake, tools, resources
and prompts) and two transports:

- stdio (newline-delimited JSON-RPC over stdin/stdout)
- streamable HTTP (a `fetch`-style handler usable with `Deno.serve`)

Enable with `--unstable-mcp`.
