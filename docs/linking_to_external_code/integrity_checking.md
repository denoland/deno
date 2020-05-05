## Integrity checking & lock files

Deno can store and check module subresource integrity for modules using a small
JSON file. Use the `--lock=lock.json` to enable and specify lock file checking.
To update or create a lock use `--lock=lock.json --lock-write`.
