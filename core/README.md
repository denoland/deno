# Deno Core

This Rust crate contains the essential V8 bindings for Deno's command-line
interface (Deno CLI). The main abstraction here is the Isolate which proivdes a
way to execute JavaScript. The Isolate is modeled as a
`Future<Item=(), Error=JSError>` which completes once all of its ops have
completed. The user must define what an Op is by implementing the `Dispatch`
trait, and by doing so define any "built-in" functionality that would be
provided by the VM. Ops are triggered by `Deno.core.dispatch()`.

Documentation for this crate is thin at the moment. Please see
[http_bench.rs](https://github.com/denoland/deno/blob/master/core/examples/http_bench.rs)
as a simple example of usage.

TypeScript support and a lot of other functionality is not available at this
layer. See the [cli](https://github.com/denoland/deno/tree/master/cli) for that.
