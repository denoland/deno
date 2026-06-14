# deno_s3

This crate implements the unstable `Deno.S3Client` and `Deno.s3` APIs, a
built-in client for S3-compatible object storage (AWS S3, MinIO, Cloudflare R2,
Backblaze B2, etc.).

It is implemented entirely in JavaScript on top of `fetch` and WebCrypto (AWS
Signature Version 4) and supports reading, writing (including multipart
uploads), deleting, stat-ing, listing and presigning objects.

Enable with `--unstable-s3`.
