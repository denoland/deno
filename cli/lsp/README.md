# Deno Language Server

The Deno Language Server provides a server implementation of the
[Language Server Protocol](https://microsoft.github.io/language-server-protocol/)
which is specifically tailored to provide a _Deno_ view of code. It is
integrated into the command line and can be started via the `lsp` sub-command.

> :warning: The Language Server is highly experimental and far from feature
> complete. This document gives an overview of the structure of the language
> server.

## Structure

When the language server is started, a `LanguageServer` instance is created
which holds all of the state of the language server. It also defines all of the
methods that the client calls via the Language Server RPC protocol.
