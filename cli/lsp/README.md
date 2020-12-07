# Deno Language Server

The Deno Language Server provides a server implementation of the
[Language Server Protocol](https://microsoft.github.io/language-server-protocol/)
which is specifically tailored to provide a _Deno_ view of code. It is
integrated into the command line and can be started via the `lsp` sub-command.

> :warning: The Language Server is highly experimental and far from feature
> complete.

This document gives an overview of the structure of the language server.

## Acknowledgement

The structure of the language server was heavily influenced and adapted from
[`rust-analyzer`](https://rust-analyzer.github.io/).

## Structure

When the language server is started, a `ServerState` instance is created which
holds all the state of the language server, as well as provides the
infrastructure for receiving and sending notifications and requests from a
language server client.
