# Add Post-Quantum Cryptography (PQC) Support to WebCrypto API

## Summary

Add support for NIST-standardized post-quantum cryptographic algorithms ML-DSA (Module-Lattice-Based Digital Signature Algorithm) and ML-KEM (Module-Lattice-Based Key-Encapsulation Mechanism) to Deno's WebCrypto API implementation.

## Motivation

Interest in PQC algorithms is growing rapidly as organizations prepare for the "quantum future." Quantum computers pose a significant threat to current public-key cryptographic systems (RSA, ECDSA, etc.). NIST has standardized ML-DSA (FIPS 204) and ML-KEM (FIPS 203) to address this threat.

Adding these algorithms to Deno would:
- Enable developers to build quantum-resistant applications today
- Position Deno as a forward-thinking runtime for secure applications
- Provide `globalThis.crypto` support for web platform compatibility
- Complement Node.js v24's existing support with a web-standard approach

## Web Standards Alignment

The W3C Web Platform Incubator Community Group (WICG) has published a draft specification:

**[Modern Algorithms in the Web Cryptography API](https://wicg.github.io/webcrypto-modern-algos/)**

This draft specification defines how ML-DSA and ML-KEM should be integrated into the WebCrypto API, including:
- Algorithm identifiers and parameters
- Key generation, import, and export formats
- Signing/verification operations (ML-DSA)
- Encapsulation/decapsulation operations (ML-KEM)

While still in draft status, this specification provides a clear path for implementation that would be compatible with other web platform implementations.

## Requested Algorithms

### ML-DSA (Digital Signatures) - FIPS 204
- **ML-DSA-44** (OID: 2.16.840.1.101.3.4.3.17) - NIST Level 2 security
- **ML-DSA-65** (OID: 2.16.840.1.101.3.4.3.18) - NIST Level 3 security
- **ML-DSA-87** (OID: 2.16.840.1.101.3.4.3.19) - NIST Level 5 security

### ML-KEM (Key Encapsulation) - FIPS 203
- **ML-KEM-512** (OID: 2.16.840.1.101.3.4.4.1) - NIST Level 1 security
- **ML-KEM-768** (OID: 2.16.840.1.101.3.4.4.2) - NIST Level 3 security
- **ML-KEM-1024** (OID: 2.16.840.1.101.3.4.4.3) - NIST Level 5 security

## API Surface

### ML-DSA (following Ed25519 pattern)

```javascript
// Generate key pair
const keyPair = await crypto.subtle.generateKey(
  { name: "ML-DSA-65" },
  true,
  ["sign", "verify"]
);

// Sign
const signature = await crypto.subtle.sign(
  { name: "ML-DSA-65" },
  keyPair.privateKey,
  data
);

// Verify
const valid = await crypto.subtle.verify(
  { name: "ML-DSA-65" },
  keyPair.publicKey,
  signature,
  data
);
```

### ML-KEM (new encapsulation API)

```javascript
// Generate key pair
const keyPair = await crypto.subtle.generateKey(
  { name: "ML-KEM-768" },
  true,
  ["encapsulateKey", "decapsulateKey"]
);

// Encapsulate (sender side)
const result = await crypto.subtle.encapsulateBits(
  { name: "ML-KEM-768" },
  keyPair.publicKey
);
// result.sharedKey: ArrayBuffer (shared secret)
// result.ciphertext: ArrayBuffer (to send to recipient)

// Decapsulate (recipient side)
const sharedKey = await crypto.subtle.decapsulateBits(
  { name: "ML-KEM-768" },
  keyPair.privateKey,
  ciphertext
);
```

## Implementation Considerations

### Current State
- Deno uses `aws-lc-rs` as its cryptographic backend (via `ext/crypto`)
- Current algorithms follow the pattern in `ext/crypto/ed25519.rs`, `ext/crypto/x25519.rs`, etc.
- JavaScript API is defined in `ext/crypto/00_crypto.js`

### Dependencies
The main blocker is cryptographic library support:

1. **AWS-LC Status**: Need to verify if `aws-lc-rs` has or plans to add ML-DSA/ML-KEM support
2. **Alternative Libraries**: If AWS-LC doesn't support these yet, could evaluate:
   - `pqcrypto` - Pure Rust PQC implementations
   - `oqs-sys` - Rust bindings to liboqs (Open Quantum Safe)
   - Direct implementation following FIPS 203/204 specifications

### Required Changes

1. **Rust modules** (in `ext/crypto/`):
   - `ml_dsa.rs` - ML-DSA key generation, signing, verification, import/export
   - `ml_kem.rs` - ML-KEM key generation, encapsulation, decapsulation, import/export

2. **JavaScript API updates** (in `ext/crypto/00_crypto.js`):
   - Add algorithm identifiers to `supportedAlgorithms`
   - Add new operations: `encapsulateKey`, `encapsulateBits`, `decapsulateKey`, `decapsulateBits`

3. **Key format support**:
   - SPKI (SubjectPublicKeyInfo) for public keys
   - PKCS#8 for private keys
   - JWK (JSON Web Key) format per [draft-ietf-jose-pqc-kem](https://datatracker.ietf.org/doc/draft-ietf-jose-pqc-kem/)
   - Raw formats (raw-public, raw-seed)

4. **Extension registration** (in `ext/crypto/lib.rs`):
   - Register new ops in the `deno_crypto` extension

## Prior Art

### Node.js v24
Node.js v24 includes support for ML-DSA and ML-KEM via the `node:crypto` module. While this could theoretically be used in Deno via npm compatibility, having native `globalThis.crypto` support would:
- Provide better web platform alignment
- Enable use in code targeting browsers and Deno
- Match developer expectations for WebCrypto API

### Browser Implementations
As the WICG spec matures, browser vendors may begin implementing these algorithms. Having Deno support them positions the runtime as a leader in quantum-resistant cryptography.

## Security Considerations

- These algorithms are **NIST-standardized** (FIPS 203, FIPS 204)
- They provide quantum resistance against attacks from future quantum computers
- Implementation must use constant-time operations to prevent timing attacks
- Key material must be handled securely and zeroized when no longer needed

## Timeline Considerations

- The WICG spec is still in **draft status** (as of January 2026)
- NIST standards (FIPS 203/204) are **finalized** (August 2024)
- Node.js already has **production implementations**
- Industry adoption is accelerating (government mandates, compliance requirements)

## Questions for Discussion

1. Should Deno wait for AWS-LC to add support, or evaluate alternative cryptographic libraries?
2. Should implementation track the WICG draft closely, or follow Node.js patterns?
3. Should ML-KEM support include the new encapsulation methods, or initially implement using existing primitives?
4. Are there performance or bundle size concerns with adding PQC algorithms?

## References

- **FIPS 203**: [Module-Lattice-Based Key-Encapsulation Mechanism Standard](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.203.pdf)
- **FIPS 204**: [Module-Lattice-Based Digital Signature Standard](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.204.pdf)
- **W3C WICG Spec**: [Modern Algorithms in the Web Cryptography API](https://wicg.github.io/webcrypto-modern-algos/)
- **Node.js Documentation**: [Crypto module PQC support](https://nodejs.org/api/crypto.html)
- **IETF Draft**: [PQ KEMs for JOSE and COSE](https://datatracker.ietf.org/doc/draft-ietf-jose-pqc-kem/)

## Related

- This would complement existing Ed25519/X25519 support
- Could be part of a broader effort to support other WICG modern algorithms (SLH-DSA, ChaCha20-Poly1305, etc.)
- May require coordination with `std/crypto` for higher-level utilities

---

I'm happy to contribute to implementation once the approach is determined. Looking forward to community feedback on this proposal!
