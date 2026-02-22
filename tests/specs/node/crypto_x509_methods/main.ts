import { createPrivateKey, X509Certificate } from "node:crypto";
import { Buffer } from "node:buffer";
import assert from "node:assert";

const certPem = `-----BEGIN CERTIFICATE-----
MIID6DCCAtCgAwIBAgIUFH02wcL3Qgben6tfIibXitsApCYwDQYJKoZIhvcNAQEL
BQAwejELMAkGA1UEBhMCVVMxCzAJBgNVBAgMAkNBMQswCQYDVQQHDAJTRjEPMA0G
A1UECgwGSm95ZW50MRAwDgYDVQQLDAdOb2RlLmpzMQwwCgYDVQQDDANjYTExIDAe
BgkqhkiG9w0BCQEWEXJ5QHRpbnljbG91ZHMub3JnMCAXDTIyMDkwMzIxNDAzN1oY
DzIyOTYwNjE3MjE0MDM3WjB9MQswCQYDVQQGEwJVUzELMAkGA1UECAwCQ0ExCzAJ
BgNVBAcMAlNGMQ8wDQYDVQQKDAZKb3llbnQxEDAOBgNVBAsMB05vZGUuanMxDzAN
BgNVBAMMBmFnZW50MTEgMB4GCSqGSIb3DQEJARYRcnlAdGlueWNsb3Vkcy5vcmcw
ggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDUVjIK+yDTgnCT3CxChO0E
37q9VuHdrlKeKLeQzUJW2yczSfNzX/0zfHpjY+zKWie39z3HCJqWxtiG2wxiOI8c
3WqWOvzVmdWADlh6EfkIlg+E7VC6JaKDA+zabmhPvnuu3JzogBMnsWl68lCXzuPx
deQAmEwNtqjrh74DtM+Ud0ulb//Ixjxo1q3rYKu+aaexSramuee6qJta2rjrB4l8
B/bU+j1mDf9XQQfSjo9jRnp4hiTFdBl2k+lZzqE2L/rhu6EMjA2IhAq/7xA2MbLo
9cObVUin6lfoo5+JKRgT9Fp2xEgDOit+2EA/S6oUfPNeLSVUqmXOSWlXlwlb9Nxr
AgMBAAGjYTBfMF0GCCsGAQUFBwEBBFEwTzAjBggrBgEFBQcwAYYXaHR0cDovL29j
c3Aubm9kZWpzLm9yZy8wKAYIKwYBBQUHMAKGHGh0dHA6Ly9jYS5ub2RlanMub3Jn
L2NhLmNlcnQwDQYJKoZIhvcNAQELBQADggEBAMM0mBBjLMt9pYXePtUeNO0VTw9y
FWCM8nAcAO2kRNwkJwcsispNpkcsHZ5o8Xf5mpCotdvziEWG1hyxwU6nAWyNOLcN
G0a0KUfbMO3B6ZYe1GwPDjXaQnv75SkAdxgX5zOzca3xnhITcjUUGjQ0fbDfwFV5
ix8mnzvfXjDONdEznVa7PFcN6QliFUMwR/h8pCRHtE5+a10OSPeJSrGG+FtrGnRW
G1IJUv6oiGF/MvWCr84REVgc1j78xomGANJIu2hN7bnD1nEMON6em8IfnDOUtynV
9wfWTqiQYD5Zifj6WcGa0aAHMuetyFG4lIfMAHmd3gaKpks7j9l26LwRPvI=
-----END CERTIFICATE-----
`;

const keyPem = `-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEA1FYyCvsg04Jwk9wsQoTtBN+6vVbh3a5Snii3kM1CVtsnM0nz
c1/9M3x6Y2Psylont/c9xwialsbYhtsMYjiPHN1qljr81ZnVgA5YehH5CJYPhO1Q
uiWigwPs2m5oT757rtyc6IATJ7FpevJQl87j8XXkAJhMDbao64e+A7TPlHdLpW//
yMY8aNat62CrvmmnsUq2prnnuqibWtq46weJfAf21Po9Zg3/V0EH0o6PY0Z6eIYk
xXQZdpPpWc6hNi/64buhDIwNiIQKv+8QNjGy6PXDm1VIp+pX6KOfiSkYE/RadsRI
AzorfthAP0uqFHzzXi0lVKplzklpV5cJW/TcawIDAQABAoIBAAvbtHfAhpjJVBgt
15rvaX04MWmZjIugzKRgib/gdq/7FTlcC+iJl85kSUF7tyGl30n62MxgwqFhAX6m
hQ6HMhbelrFFIhGbwbyhEHfgwROlrcAysKt0pprCgVvBhrnNXYLqdyjU3jz9P3LK
TY3s0/YMK2uNFdI+PTjKH+Z9Foqn9NZUnUonEDepGyuRO7fLeccWJPv2L4CR4a/5
ku4VbDgVpvVSVRG3PSVzbmxobnpdpl52og+T7tPx1cLnIknPtVljXPWtZdfekh2E
eAp2KxCCHOKzzG3ItBKsVu0woeqEpy8JcoO6LbgmEoVnZpgmtQClbBgef8+i+oGE
BgW9nmECgYEA8gA63QQuZOUC56N1QXURexN2PogF4wChPaCTFbQSJXvSBkQmbqfL
qRSD8P0t7GOioPrQK6pDwFf4BJB01AvkDf8Z6DxxOJ7cqIC7LOwDupXocWX7Q0Qk
O6cwclBVsrDZK00v60uRRpl/a39GW2dx7IiQDkKQndLh3/0TbMIWHNcCgYEA4J6r
yinZbLpKw2+ezhi4B4GT1bMLoKboJwpZVyNZZCzYR6ZHv+lS7HR/02rcYMZGoYbf
n7OHwF4SrnUS7vPhG4g2ZsOhKQnMvFSQqpGmK1ZTuoKGAevyvtouhK/DgtLWzGvX
9fSahiq/UvfXs/z4M11q9Rv9ztPCmG1cwSEHlo0CgYEAogQNZJK8DMhVnYcNpXke
7uskqtCeQE/Xo06xqkIYNAgloBRYNpUYAGa/vsOBz1UVN/kzDUi8ezVp0oRz8tLT
J5u2WIi+tE2HJTiqF3UbOfvK1sCT64DfUSCpip7GAQ/tFNRkVH8PD9kMOYfILsGe
v+DdsO5Xq5HXrwHb02BNNZkCgYBsl8lt33WiPx5OBfS8pu6xkk+qjPkeHhM2bKZs
nkZlS9j0KsudWGwirN/vkkYg8zrKdK5AQ0dqFRDrDuasZ3N5IA1M+V88u+QjWK7o
B6pSYVXxYZDv9OZSpqC+vUrEQLJf+fNakXrzSk9dCT1bYv2Lt6ox/epix7XYg2bI
Z/OHMQKBgQC2FUGhlndGeugTJaoJ8nhT/0VfRUX/h6sCgSerk5qFr/hNCBV4T022
x0NDR2yLG6MXyqApJpG6rh3QIDElQoQCNlI3/KJ6JfEfmqrLLN2OigTvA5sE4fGU
Dp/ha8OQAx95EwXuaG7LgARduvOIK3x8qi8KsZoUGJcg2ywurUbkWA==
-----END RSA PRIVATE KEY-----
`;

const caPem = `-----BEGIN CERTIFICATE-----
MIIDlDCCAnygAwIBAgIUSrFsjf1qfQ0t/KvfnEsOksatAikwDQYJKoZIhvcNAQEL
BQAwejELMAkGA1UEBhMCVVMxCzAJBgNVBAgMAkNBMQswCQYDVQQHDAJTRjEPMA0G
A1UECgwGSm95ZW50MRAwDgYDVQQLDAdOb2RlLmpzMQwwCgYDVQQDDANjYTExIDAe
BgkqhkiG9w0BCQEWEXJ5QHRpbnljbG91ZHMub3JnMCAXDTIyMDkwMzIxNDAzN1oY
DzIyOTYwNjE3MjE0MDM3WjB6MQswCQYDVQQGEwJVUzELMAkGA1UECAwCQ0ExCzAJ
BgNVBAcMAlNGMQ8wDQYDVQQKDAZKb3llbnQxEDAOBgNVBAsMB05vZGUuanMxDDAK
BgNVBAMMA2NhMTEgMB4GCSqGSIb3DQEJARYRcnlAdGlueWNsb3Vkcy5vcmcwggEi
MA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDNvf4OGGep+ak+4DNjbuNgy0S/
AZPxahEFp4gpbcvsi9YLOPZ31qpilQeQf7d27scIZ02Qx1YBAzljxELB8H/ZxuYS
cQK0s+DNP22xhmgwMWznO7TezkHP5ujN2UkbfbUpfUxGFgncXeZf9wR7yFWppeHi
RWNBOgsvY7sTrS12kXjWGjqntF7xcEDHc7h+KyF6ZjVJZJCnP6pJEQ+rUjd51eCZ
Xt4WjowLnQiCS1VKzXiP83a++Ma1BKKkUitTR112/Uwd5eGoiByhmLzb/BhxnHJN
07GXjhlMItZRm/jfbZsx1mwnNOO3tx4r08l+DaqkinIadvazs+1ugCaKQn8xAgMB
AAGjEDAOMAwGA1UdEwQFMAMBAf8wDQYJKoZIhvcNAQELBQADggEBAFqG0RXURDam
56x5accdg9sY5zEGP5VQhkK3ZDc2NyNNa25rwvrjCpO+e0OSwKAmm4aX6iIf2woY
wF2f9swWYzxn9CG4fDlUA8itwlnHxupeL4fGMTYb72vf31plUXyBySRsTwHwBloc
F7KvAZpYYKN9EMH1S/267By6H2I33BT/Ethv//n8dSfmuCurR1kYRaiOC4PVeyFk
B3sj8TtolrN0y/nToWUhmKiaVFnDx3odQ00yhmxR3t21iB7yDkko6D8Vf2dVC4j/
YYBVprXGlTP/hiYRLDoP20xKOYznx5cvHPJ9p+lVcOZUJsJj/Iy750+2n5UiBmXt
lz88C25ucKA=
-----END CERTIFICATE-----
`;

const x509 = new X509Certificate(certPem);
const privateKey = createPrivateKey(keyPem);
const caCert = new X509Certificate(caPem);

// Test toString()
const pemStr = x509.toString();
assert(
  pemStr.startsWith("-----BEGIN CERTIFICATE-----\n"),
  "toString should start with PEM header",
);
assert(
  pemStr.endsWith("-----END CERTIFICATE-----\n"),
  "toString should end with PEM footer",
);
assert.strictEqual(x509.toJSON(), x509.toString());
console.log("toString: OK");

// Test raw
const raw = x509.raw;
assert(Buffer.isBuffer(raw), "raw should be a Buffer");
assert(raw.length > 0, "raw should not be empty");
console.log("raw: OK");

// Test subjectAltName (agent1-cert has no SAN)
assert.strictEqual(x509.subjectAltName, undefined);
console.log("subjectAltName: OK");

// Test checkIP
assert.strictEqual(x509.checkIP("127.0.0.1"), undefined);
assert.strictEqual(x509.checkIP("::"), undefined);
console.log("checkIP: OK");

// Test checkIssued
assert.strictEqual(x509.checkIssued(caCert), true);
assert.strictEqual(x509.checkIssued(x509), false);
try {
  (x509 as any).checkIssued({});
  assert.fail("Should throw");
} catch (e: any) {
  assert.strictEqual(e.code, "ERR_INVALID_ARG_TYPE");
}
console.log("checkIssued: OK");

// Test checkPrivateKey
assert.strictEqual(x509.checkPrivateKey(privateKey), true);
try {
  (x509 as any).checkPrivateKey(x509.publicKey);
  assert.fail("Should throw");
} catch (e: any) {
  assert.strictEqual(e.code, "ERR_INVALID_ARG_VALUE");
}
console.log("checkPrivateKey: OK");

// Test verify
assert.strictEqual(x509.verify(caCert.publicKey), true);
assert.strictEqual(x509.verify(x509.publicKey), false);
try {
  (x509 as any).verify(privateKey);
  assert.fail("Should throw");
} catch (e: any) {
  assert.strictEqual(e.code, "ERR_INVALID_ARG_VALUE");
}
console.log("verify: OK");

// Test infoAccess
const infoAccess = x509.infoAccess;
assert(infoAccess !== undefined, "infoAccess should not be undefined");
assert(
  infoAccess!.includes("OCSP - URI:http://ocsp.nodejs.org/"),
  "infoAccess should contain OCSP URI",
);
assert(
  infoAccess!.includes("CA Issuers - URI:http://ca.nodejs.org/ca.cert"),
  "infoAccess should contain CA Issuers URI",
);
console.log("infoAccess: OK");

// Test toLegacyObject
const legacy = x509.toLegacyObject();
assert(legacy !== undefined, "toLegacyObject should return an object");
assert(legacy.subject !== undefined, "legacy object should have subject");
assert(legacy.issuer !== undefined, "legacy object should have issuer");
console.log("toLegacyObject: OK");

console.log("All X509Certificate method tests passed!");
