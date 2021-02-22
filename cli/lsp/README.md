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

## Custom requests

The LSP currently supports the following custom requests. A client should
implement these in order to have a fully functioning client that integrates well
with Deno:

- `deno/cache` - This command will instruct Deno to attempt to cache a module
  and all of its dependencies. If a `referrer` only is passed, then all
  dependencies for the module specifier will be loaded. If there are values in
  the `uris`, then only those `uris` will be cached.

  It expects parameters of:

  ```ts
  interface CacheParams {
    referrer: TextDocumentIdentifier;
    uris: TextDocumentIdentifier[];
  }
  ```
- `deno/performance` - Requests the return of the timing averages for the
  internal instrumentation of Deno.

  It does not expect any parameters.
- `deno/virtualTextDocument` - Requests a virtual text document from the LSP,
  which is a read only document that can be displayed in the client. This allows
  clients to access documents in the Deno cache, like remote modules and
  TypeScript library files built into Deno. The Deno language server will encode
  all internal files under the custom schema `deno:`, so clients should route
  all requests for the `deno:` schema back to the `deno/virtualTextDocument`
  API.

  It also supports a special URL of `deno:/status.md` which provides a markdown
  formatted text document that contains details about the status of the LSP for
  display to a user.

  It expects parameters of:

  ```ts
  interface VirtualTextDocumentParams {
    textDocument: TextDocumentIdentifier;
  }
  ```
