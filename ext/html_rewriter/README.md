# deno_html_rewriter

This crate implements the `HTMLRewriter` API for Deno, a streaming HTML
transformation API compatible with the one available in Cloudflare Workers and
Bun. It is backed by the [`lol_html`](https://crates.io/crates/lol-html) crate.

The API is available behind the `--unstable-html-rewriter` flag.

Spec: https://developers.cloudflare.com/workers/runtime-apis/html-rewriter/
