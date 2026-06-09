# deno_crypto

**This crate implements the Web Cryptography API.**

Spec: https://www.w3.org/TR/WebCryptoAPI/

## Usage Example

From javascript, include the extension's source, and assign `CryptoKey`,
`crypto`, `Crypto`, and `SubtleCrypto` to the global scope:

```javascript
import { core } from "ext:core/mod.js";

const crypto = core.loadExtScript("ext:deno_crypto/00_crypto.js");

Object.defineProperty(globalThis, "CryptoKey", {
  value: crypto.CryptoKey,
  enumerable: false,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "crypto", {
  value: crypto.crypto,
  enumerable: false,
  configurable: true,
  writable: false,
});

Object.defineProperty(globalThis, "Crypto", {
  value: crypto.Crypto,
  enumerable: false,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "SubtleCrypto", {
  value: crypto.SubtleCrypto,
  enumerable: false,
  configurable: true,
  writable: true,
});
```

Then from rust, provide: `deno_crypto::deno_crypto::init(Option<u64>)` in the
`extensions` field of your `RuntimeOptions`

Where the `Option<u64>` represents an optional seed for initialization.

## Dependencies

- **deno_webidl**: Provided by the `deno_webidl` crate
- **deno_web**: Provided by the `deno_web` crate

## Provided ops

Following ops are provided, which can be accessed through `Deno.ops`:

- op_crypto_get_random_values
- op_crypto_generate_key
- op_crypto_sign_key
- op_crypto_verify_key
- op_crypto_derive_bits
- op_crypto_import_key
- op_crypto_export_key
- op_crypto_encrypt
- op_crypto_decrypt
- op_crypto_subtle_digest
- op_crypto_subtle_digest_xof
- op_crypto_random_uuid
- op_crypto_wrap_key
- op_crypto_unwrap_key
- op_crypto_base64url_decode
- op_crypto_base64url_encode
- key_store::op_crypto_key_store_insert
- key_store::op_crypto_key_store_get
- x25519::op_crypto_generate_x25519_keypair
- x25519::op_crypto_x25519_public_key
- x25519::op_crypto_derive_bits_x25519
- x25519::op_crypto_import_spki_x25519
- x25519::op_crypto_import_pkcs8_x25519
- x25519::op_crypto_export_spki_x25519
- x25519::op_crypto_export_pkcs8_x25519
- x448::op_crypto_generate_x448_keypair
- x448::op_crypto_derive_bits_x448
- x448::op_crypto_import_spki_x448
- x448::op_crypto_import_pkcs8_x448
- x448::op_crypto_x448_public_key
- x448::op_crypto_export_spki_x448
- x448::op_crypto_export_pkcs8_x448
- ed25519::op_crypto_generate_ed25519_keypair
- ed25519::op_crypto_import_spki_ed25519
- ed25519::op_crypto_import_pkcs8_ed25519
- ed25519::op_crypto_sign_ed25519
- ed25519::op_crypto_verify_ed25519
- ed25519::op_crypto_export_spki_ed25519
- ed25519::op_crypto_export_pkcs8_ed25519
- ed25519::op_crypto_jwk_x_ed25519
- mldsa::op_crypto_mldsa_from_seed
- mldsa::op_crypto_mldsa_from_pkcs8
- mldsa::op_crypto_mldsa_from_spki
- mldsa::op_crypto_mldsa_export_pkcs8
- mldsa::op_crypto_mldsa_export_spki
- mldsa::op_crypto_sign_mldsa
- mldsa::op_crypto_verify_mldsa
- mlkem::op_crypto_ml_kem_from_seed
- mlkem::op_crypto_ml_kem_encapsulate
- mlkem::op_crypto_ml_kem_decapsulate
- mlkem::op_crypto_ml_kem_import_spki
- mlkem::op_crypto_ml_kem_import_pkcs8
- mlkem::op_crypto_ml_kem_export_spki
- mlkem::op_crypto_ml_kem_export_pkcs8
- mlkem::op_crypto_ml_kem_get_public_key
- mlkem::op_crypto_ml_kem_validate_public_key
