The certificates in this dir expire on Sept, 27th, 2118

Certificates generated using original instructions from this gist:
https://gist.github.com/cecilemuller/9492b848eb8fe46d462abeb26656c4f8

## Certificate authority (CA)

Generate RootCA.pem, RootCA.key, RootCA.crt:

```shell
openssl req -x509 -nodes -new -sha256 -days 36135 -newkey rsa:2048 -keyout RootCA.key -out RootCA.pem -subj "/C=US/CN=Example-Root-CA"
openssl x509 -outform pem -in RootCA.pem -out RootCA.crt
```

Note that Example-Root-CA is an example, you can customize the name.

## Domain name certificate

First, create a file domains.txt that lists all your local domains (here we only
list localhost):

```shell
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
subjectAltName = @alt_names
[alt_names]
DNS.1 = localhost
```

Generate localhost.key, localhost.csr, and localhost.crt:

```shell
openssl req -new -nodes -newkey rsa:2048 -keyout localhost.key -out localhost.csr -subj "/C=US/ST=YourState/L=YourCity/O=Example-Certificates/CN=localhost.local"
openssl x509 -req -sha256 -days 36135 -in localhost.csr -CA RootCA.pem -CAkey RootCA.key -CAcreateserial -extfile domains.txt -out localhost.crt
```

Note that the country / state / city / name in the first command can be
customized.

Generate localhost_ecc.key, localhost_ecc.csr, and localhost_ecc.crt:

```shell
openssl ecparam -genkey -name prime256v1 -noout --out localhost_ecc.key
openssl req -new -key localhost_ecc.key -out localhost_ecc.csr -subj "/C=US/ST=YourState/L=YourCity/O=Example-Certificates/CN=localhost.local"
openssl x509 -req -sha256 -days 36135 -in localhost_ecc.csr -CA RootCA.pem -CAkey RootCA.key -CAcreateserial -extfile domains.txt -out localhost_ecc.crt
```

For testing purposes we need following files:

- `RootCA.crt`
- `RootCA.key`
- `RootCA.pem`
- `localhost.crt`
- `localhost.key`
- `localhost_ecc.crt`
- `localhost_ecc.key`

## PKCS#12 (PFX) bundles

Bundles wrapping `localhost.crt` + `localhost.key`, with the MAC computed using
each of the supported hash algorithms. All use the passphrase `secret`.

```shell
for alg in sha1 sha256 sha384 sha512; do
  openssl pkcs12 -export -macalg "$alg" \
    -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES \
    -inkey localhost.key -in localhost.crt \
    -passout pass:secret \
    -out "localhost_${alg}.pfx"
done
```

`-keypbe`/`-certpbe` pin bag encryption to legacy PBE-SHA1-3DES so the fixtures
exercise the PKCS#12 PBE path in `op_node_load_pfx`; the PBES2 + AES-256-CBC
path (OpenSSL 3.x's default) is covered separately by `localhost_modern.pfx`.

- `localhost_sha1.pfx` — RFC 7292 default MAC algorithm
- `localhost_sha256.pfx` — OpenSSL 3.x default MAC algorithm
- `localhost_sha384.pfx`
- `localhost_sha512.pfx`

A separate bundle uses OpenSSL 3.x's default PBES2 + PBKDF2 + AES-256-CBC shape
(regression coverage for #34434). Generated with:

```shell
openssl pkcs12 -export \
  -inkey localhost.key -in localhost.crt \
  -passout pass:secret \
  -out localhost_modern.pfx
```

- `localhost_modern.pfx` — passphrase `secret`, PBES2/AES-256-CBC bags

A variant without a MAC (`-nomac`) checks that MAC-less PFX files are still
accepted, and that a wrong passphrase surfaces as a key-decrypt failure rather
than a MAC failure (the certs are plaintext, only the key is shrouded).
Generated with:

```shell
openssl pkcs12 -export \
  -inkey localhost.key -in localhost.crt \
  -passout pass:secret -nomac \
  -out localhost_modern_nomac.pfx
```

- `localhost_modern_nomac.pfx` — passphrase `secret`, PBES2 shrouded key, no MAC

A bundle carrying a CA chain (`-certfile RootCA.pem`) has more than one cert
bag, so it exercises the leaf-vs-CA split in `op_node_load_pfx` (first bag is
the leaf, the rest become the `ca` chain) and decrypting an EncryptedData
envelope that holds multiple cert bags. Generated with:

```shell
openssl pkcs12 -export \
  -inkey localhost.key -in localhost.crt \
  -certfile RootCA.pem \
  -passout pass:secret \
  -out localhost_modern_chain.pfx
```

- `localhost_modern_chain.pfx` — passphrase `secret`, leaf `localhost.crt` +
  `RootCA` chain, PBES2/AES-256-CBC bags

A separate self-signed bundle exercises the `DEPTH_ZERO_SELF_SIGNED_CERT` path
in the TLS handshake. Generated with:

```shell
openssl req -x509 -nodes -newkey rsa:2048 \
  -keyout localhost_ss.key -out localhost_ss.crt \
  -days 36135 -sha256 \
  -subj "/C=US/CN=localhost" \
  -addext "subjectAltName=DNS:localhost"
openssl pkcs12 -export \
  -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES \
  -inkey localhost_ss.key -in localhost_ss.crt \
  -passout pass:testpass \
  -out localhost.pfx
```

- `localhost.pfx` — self-signed `CN=localhost`, passphrase `testpass`
