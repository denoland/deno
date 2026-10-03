# ext/crypto test fixtures

RSA keys for the `awslc_sign_verify` boundary tests. Checked in rather than
generated at test time because RustCrypto keygen is far too slow in debug
builds, and the 8192/9216-bit sizes exist precisely to pin the aws-lc fast-path
modulus range (2048-8192 bits) from both sides.

Generated with LibreSSL:

```sh
openssl genrsa -out rsa<bits>.pem <bits>
# PKCS#1 RSAPrivateKey DER (the shape stored for private CryptoKeys)
openssl rsa -in rsa<bits>.pem -outform DER -out rsa<bits>_private_pkcs1.der
# PKCS#1 RSAPublicKey DER (the shape stored for public CryptoKeys)
openssl rsa -in rsa<bits>.pem -RSAPublicKey_out -outform DER -out rsa<bits>_public_pkcs1.der
# RSASSA-PKCS1-v1_5 SHA-256 signature over the exact string
# "deno ext/crypto sign/verify fixture" (no trailing newline)
printf 'deno ext/crypto sign/verify fixture' > payload.bin
openssl dgst -sha256 -sign rsa<bits>.pem -out rsa<bits>_sig_sha256.bin payload.bin
```

The SPKI encodings of the 8192/9216-bit public keys
(`openssl rsa -in rsa<bits>.pem -pubout -outform DER`) and their signature files
live in `tests/testdata/webcrypto/` for `tests/unit/webcrypto_test.ts`.
