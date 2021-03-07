# deno crypto

Op crate that implements the WebCrypto API functions.

## Supported algorithms

SubtleCrypto for Deno is in it's early stages; below is the supported algorithms
and functions.

| Algorithm name                                                        | generateKey        | sign               |
| --------------------------------------------------------------------- | ------------------ | ------------------ |
| [RSASSA-PKCS1-v1_5](https://www.w3.org/TR/WebCryptoAPI/#rsassa-pkcs1) | :white_check_mark: | :white_check_mark: |
| [RSA-PSS](https://www.w3.org/TR/WebCryptoAPI/#rsa-pss)                | :white_check_mark: | :white_check_mark: |
| [RSA-OAEP](https://www.w3.org/TR/WebCryptoAPI/#rsa-oaep)              | :white_check_mark: |                    |
| [ECDSA](https://www.w3.org/TR/WebCryptoAPI/#ecdsa)                    | :white_check_mark: | :white_check_mark: |
| [ECDH](https://www.w3.org/TR/WebCryptoAPI/#ecdh)                      | :white_check_mark: |                    |
| [HMAC](https://www.w3.org/TR/WebCryptoAPI/#hmac)                      | :white_check_mark: | :white_check_mark: |

All WebCrypto algorithms can be found
[here](https://www.w3.org/TR/WebCryptoAPI/#algorithm-overview).
