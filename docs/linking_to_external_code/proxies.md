## Proxies

Deno supports proxies for module downloads and `fetch` API.

Proxy configuration is read from environmental variables: `HTTP_PROXY` and
`HTTPS_PROXY`.

In case of Windows if environmental variables are not found Deno falls back to
reading proxies from registry.
