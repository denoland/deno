// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 crypto-browserify. All rights reserved. MIT license.
// from https://github.com/crypto-browserify/parse-asn1/blob/fbd70dca8670d17955893e083ca69118908570be/certificate.js

import * as asn from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/mod.js";

const Time = asn.define("Time", function () {
  this.choice({
    utcTime: this.utctime(),
    generalTime: this.gentime(),
  });
});

const AttributeTypeValue = asn.define("AttributeTypeValue", function () {
  this.seq().obj(
    this.key("type").objid(),
    this.key("value").any(),
  );
});

const AlgorithmIdentifier = asn.define("AlgorithmIdentifier", function () {
  this.seq().obj(
    this.key("algorithm").objid(),
    this.key("parameters").optional(),
    this.key("curve").objid().optional(),
  );
});

const SubjectPublicKeyInfo = asn.define("SubjectPublicKeyInfo", function () {
  this.seq().obj(
    this.key("algorithm").use(AlgorithmIdentifier),
    this.key("subjectPublicKey").bitstr(),
  );
});

const RelativeDistinguishedName = asn.define(
  "RelativeDistinguishedName",
  function () {
    this.setof(AttributeTypeValue);
  },
);

const RDNSequence = asn.define("RDNSequence", function () {
  this.seqof(RelativeDistinguishedName);
});

const Name = asn.define("Name", function () {
  this.choice({
    rdnSequence: this.use(RDNSequence),
  });
});

const Validity = asn.define("Validity", function () {
  this.seq().obj(
    this.key("notBefore").use(Time),
    this.key("notAfter").use(Time),
  );
});

const Extension = asn.define("Extension", function () {
  this.seq().obj(
    this.key("extnID").objid(),
    this.key("critical").bool().def(false),
    this.key("extnValue").octstr(),
  );
});

const TBSCertificate = asn.define("TBSCertificate", function () {
  this.seq().obj(
    this.key("version").explicit(0).int().optional(),
    this.key("serialNumber").int(),
    this.key("signature").use(AlgorithmIdentifier),
    this.key("issuer").use(Name),
    this.key("validity").use(Validity),
    this.key("subject").use(Name),
    this.key("subjectPublicKeyInfo").use(SubjectPublicKeyInfo),
    this.key("issuerUniqueID").implicit(1).bitstr().optional(),
    this.key("subjectUniqueID").implicit(2).bitstr().optional(),
    this.key("extensions").explicit(3).seqof(Extension).optional(),
  );
});

export const X509Certificate = asn.define("X509Certificate", function () {
  this.seq().obj(
    this.key("tbsCertificate").use(TBSCertificate),
    this.key("signatureAlgorithm").use(AlgorithmIdentifier),
    this.key("signatureValue").bitstr(),
  );
});

export default X509Certificate;
