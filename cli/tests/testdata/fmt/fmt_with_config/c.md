## Permissions

Deno is secure by default. Therefore,
unless you specifically enable it, a
program run with Deno has no file,
network, or environment access. Access
to security sensitive functionality
requires that permisisons have been
granted to an executing script through
command line flags, or a runtime
permission prompt.

For the following example `mod.ts` has
been granted read-only access to the
file system. It cannot write to the file
system, or perform any other security
sensitive functions.
