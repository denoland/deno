# How it works

## base64

#### convertBase64ToUint8Array(data: string): Uint8Array

Converts a base64 encoded ascii string to a Uint8Array.

#### convertUint8ArrayToBase64(data: Uint8Array): string

Converts a Uint8Array to a base64 encoded ascii string.

#### convertStringToBase64(str: string): string

Takes a ucs-2 string and returns a base64 encoded ascii string.

#### convertBase64ToString(str: string): string

Takes a base64 encoded ascii string and returns a ucs-2 string.

### Example

```typescript
const str = "Hello ☸☹☺☻☼☾☿ World ✓"
const uint8Array = new TextEncoder().encode(str)
uint8Array === convertBase64ToUint8Array(convertUint8ArrayToBase64(uint8Array))
str === convertBase64ToString(convertStringToBase64(str))
```

## base64url

#### convertBase64ToBase64url(base64: string): string

The `+` and `/` characters of standard **base64** are respectively replaced by
`-` and `_` and the padding `=` characters are removed.

#### convertBase64urlToBase64(base64url: string): string

Converts a **base64url** string to standard **base64**.

### Example

```typescript
const base64 = "c3ViamVjdHM/X2Q9MQ=="
const base64url = convertBase64ToBase64url(base64)

console.log(base64url) // c3ViamVjdHM_X2Q9MQ
```