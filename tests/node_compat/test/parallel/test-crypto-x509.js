// deno-fmt-ignore-file
// deno-lint-ignore-file

// Copyright Joyent and Node contributors. All rights reserved. MIT license.

'use strict';
const common = require('../common');

if (!common.hasCrypto)
  common.skip('missing crypto');

const {
  X509Certificate,
} = require('crypto');

const assert = require('assert');
const fixtures = require('../common/fixtures');
const { readFileSync } = require('fs');

const cert = readFileSync(fixtures.path('keys', 'agent1-cert.pem'));
const ca = readFileSync(fixtures.path('keys', 'ca1-cert.pem'));

[1, {}, false, null].forEach((i) => {
  assert.throws(() => new X509Certificate(i), {
    code: 'ERR_INVALID_ARG_TYPE'
  });
});

const subjectCheck = `C=US
ST=CA
L=SF
O=Joyent
OU=Node.js
CN=agent1
Email=ry@tinyclouds.org`;

const issuerCheck = `C=US
ST=CA
L=SF
O=Joyent
OU=Node.js
CN=ca1
Email=ry@tinyclouds.org`;

let infoAccessCheck = `OCSP - URI:http://ocsp.nodejs.org/
CA Issuers - URI:http://ca.nodejs.org/ca.cert`;
if (!common.hasOpenSSL3)
  infoAccessCheck += '\n';

const der = Buffer.from(
  '308203e8308202d0a0030201020214147d36c1c2f74206de9fab5f2226d78adb00a42630' +
  '0d06092a864886f70d01010b0500307a310b3009060355040613025553310b3009060355' +
  '04080c024341310b300906035504070c025346310f300d060355040a0c064a6f79656e74' +
  '3110300e060355040b0c074e6f64652e6a73310c300a06035504030c036361313120301e' +
  '06092a864886f70d010901161172794074696e79636c6f7564732e6f72673020170d3232' +
  '303930333231343033375a180f32323936303631373231343033375a307d310b30090603' +
  '55040613025553310b300906035504080c024341310b300906035504070c025346310f30' +
  '0d060355040a0c064a6f79656e743110300e060355040b0c074e6f64652e6a73310f300d' +
  '06035504030c066167656e74313120301e06092a864886f70d010901161172794074696e' +
  '79636c6f7564732e6f726730820122300d06092a864886f70d01010105000382010f0030' +
  '82010a0282010100d456320afb20d3827093dc2c4284ed04dfbabd56e1ddae529e28b790' +
  'cd4256db273349f3735ffd337c7a6363ecca5a27b7f73dc7089a96c6d886db0c62388f1c' +
  'dd6a963afcd599d5800e587a11f908960f84ed50ba25a28303ecda6e684fbe7baedc9ce8' +
  '801327b1697af25097cee3f175e400984c0db6a8eb87be03b4cf94774ba56fffc8c63c68' +
  'd6adeb60abbe69a7b14ab6a6b9e7baa89b5adab8eb07897c07f6d4fa3d660dff574107d2' +
  '8e8f63467a788624c574197693e959cea1362ffae1bba10c8c0d88840abfef103631b2e8' +
  'f5c39b5548a7ea57e8a39f89291813f45a76c448033a2b7ed8403f4baa147cf35e2d2554' +
  'aa65ce49695797095bf4dc6b0203010001a361305f305d06082b06010505070101045130' +
  '4f302306082b060105050730018617687474703a2f2f6f6373702e6e6f64656a732e6f72' +
  '672f302806082b06010505073002861c687474703a2f2f63612e6e6f64656a732e6f7267' +
  '2f63612e63657274300d06092a864886f70d01010b05000382010100c3349810632ccb7d' +
  'a585de3ed51e34ed154f0f7215608cf2701c00eda444dc2427072c8aca4da6472c1d9e68' +
  'f177f99a90a8b5dbf3884586d61cb1c14ea7016c8d38b70d1b46b42947db30edc1e9961e' +
  'd46c0f0e35da427bfbe52900771817e733b371adf19e12137235141a34347db0dfc05579' +
  '8b1f269f3bdf5e30ce35d1339d56bb3c570de9096215433047f87ca42447b44e7e6b5d0e' +
  '48f7894ab186f85b6b1a74561b520952fea888617f32f582afce1111581cd63efcc68986' +
  '00d248bb684dedb9c3d6710c38de9e9bc21f9c3394b729d5f707d64ea890603e5989f8fa' +
  '59c19ad1a00732e7adc851b89487cc00799dde068aa64b3b8fd976e8bc113ef2',
  'hex');

{
  const x509 = new X509Certificate(cert);

  assert(!x509.ca);
  assert.strictEqual(x509.subject, subjectCheck);
  assert.strictEqual(x509.subjectAltName, undefined);
  assert.strictEqual(x509.issuer, issuerCheck);
  assert.strictEqual(x509.validFrom, 'Sep  3 21:40:37 2022 +00:00');
  assert.strictEqual(x509.validTo, 'Jun 17 21:40:37 2296 +00:00');
  assert.strictEqual(
    x509.fingerprint,
    '8B:89:16:C4:99:87:D2:13:1A:64:94:36:38:A5:32:01:F0:95:3B:53');
  assert.strictEqual(
    x509.fingerprint256,
    '2C:62:59:16:91:89:AB:90:6A:3E:98:88:A6:D3:C5:58:58:6C:AE:FF:9C:33:' +
    '22:7C:B6:77:D3:34:E7:53:4B:05'
  );
  assert.strictEqual(
    x509.fingerprint512,
    '0B:6F:D0:4D:6B:22:53:99:66:62:51:2D:2C:96:F2:58:3F:95:1C:CC:4C:44:' +
    '9D:B5:59:AA:AD:A8:F6:2A:24:8A:BB:06:A5:26:42:52:30:A3:37:61:30:A9:' +
    '5A:42:63:E0:21:2F:D6:70:63:07:96:6F:27:A7:78:12:08:02:7A:8B'
  );
  assert.strictEqual(x509.keyUsage, undefined);
  assert.strictEqual(x509.serialNumber, '147D36C1C2F74206DE9FAB5F2226D78ADB00A426');

  assert.strictEqual(x509.checkEmail('ry@tinyclouds.org'), 'ry@tinyclouds.org');
  assert.strictEqual(x509.checkEmail('sally@example.com'), undefined);
}