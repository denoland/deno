#!/usr/bin/env -S deno run --allow-write --allow-read --allow-run --allow-net --config=tests/config/deno.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console
import { join } from "jsr:@std/path/join";

const nodeVersion = "v20.11.1";
const url =
  `https://raw.githubusercontent.com/nodejs/node/${nodeVersion}/src/node_root_certs.h`;
const tlsPath = join(import.meta.dirname, "../ext/node/polyfills/tls.ts");

console.log("Fetching certs from" + url);
const nodeSourcecode = await (await fetch(url)).text();
const rootCerts = transformCppToJs(nodeSourcecode);
updateRootCertificates(tlsPath, rootCerts);

function transformCppToJs(cppCode) {
  // Remove C++ preprocessor directives
  let jsCode = cppCode.replace(/#.*$/gm, "");
  // Extract certificate strings with their comments
  const certRegex =
    /(\/\*[\s\S]*?\*\/)\s*"-----BEGIN CERTIFICATE-----\s*([\s\S]*?)\s*-----END CERTIFICATE-----"/g;
  const certificates = [];
  let match;

  while ((match = certRegex.exec(jsCode)) !== null) {
    let cert = match[0].substring(match[1].length).trim();
    // Split the certificate into lines, add '+' at the end of each line, and rejoin
    cert = cert.split("\n").map((line) => line.trim() ? `${line.trim()} +` : "")
      .join("\n");
    // Remove the last '+'
    cert = cert.slice(0, -1).trim();
    certificates.push(cert);
  }

  jsCode = "export const rootCertificates = [\n";
  jsCode += certificates.join(",\n\n");
  jsCode += "\n];";

  return jsCode;
}

function updateRootCertificates(filePath, certs) {
  const fileContent = Deno.readTextFileSync(filePath);

  const startMarker = "// -- ROOT_CERTIFICATES_START --";
  const endMarker = "// -- ROOT_CERTIFICATES_END --";
  const startIndex = fileContent.indexOf(startMarker);
  const endIndex = fileContent.indexOf(endMarker);
  if (startIndex === -1 || endIndex === -1) {
    throw new Error("Start or end marker not found in the file");
  }

  const updatedContent =
    fileContent.substring(0, startIndex + startMarker.length) +
    "\n" + certs + "\n" +
    fileContent.substring(endIndex);
  Deno.writeTextFileSync(filePath, updatedContent);

  console.log("Root certificates updated");
}
