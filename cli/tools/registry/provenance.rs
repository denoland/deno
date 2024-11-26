// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::http_util;
use crate::http_util::HttpClient;

use super::api::OidcTokenResponse;
use super::auth::gha_oidc_token;
use super::auth::is_gha;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::prelude::BASE64_STANDARD;
use base64::Engine as _;
use deno_core::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use http_body_util::BodyExt;
use once_cell::sync::Lazy;
use p256::elliptic_curve;
use p256::pkcs8::AssociatedOid;
use ring::rand::SystemRandom;
use ring::signature::EcdsaKeyPair;
use ring::signature::KeyPair;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use spki::der::asn1;
use spki::der::pem::LineEnding;
use spki::der::EncodePem;
use std::collections::HashMap;
use std::env;

const PAE_PREFIX: &str = "DSSEv1";

/// DSSE Pre-Auth Encoding
///
/// https://github.com/secure-systems-lab/dsse/blob/master/protocol.md#signature-definition
fn pre_auth_encoding(payload_type: &str, payload: &str) -> Vec<u8> {
  format!(
    "{} {} {} {} {}",
    PAE_PREFIX,
    payload_type.len(),
    payload_type,
    payload.len(),
    payload,
  )
  .into_bytes()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Signature {
  keyid: &'static str,
  sig: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
  payload_type: String,
  payload: String,
  signatures: Vec<Signature>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureBundle {
  #[serde(rename = "$case")]
  case: &'static str,
  dsse_envelope: Envelope,
}

#[derive(Serialize)]
pub struct SubjectDigest {
  pub sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
  pub name: String,
  pub digest: SubjectDigest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GhaResourceDigest {
  git_commit: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GithubInternalParameters {
  event_name: String,
  repository_id: String,
  repository_owner_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceDescriptor {
  uri: String,
  digest: Option<GhaResourceDigest>,
}

#[derive(Serialize)]
struct InternalParameters {
  github: GithubInternalParameters,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GhaWorkflow {
  #[serde(rename = "ref")]
  ref_: String,
  repository: String,
  path: String,
}

#[derive(Serialize)]
struct ExternalParameters {
  workflow: GhaWorkflow,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildDefinition {
  build_type: &'static str,
  resolved_dependencies: [ResourceDescriptor; 1],
  internal_parameters: InternalParameters,
  external_parameters: ExternalParameters,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Builder {
  id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
  invocation_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunDetails {
  builder: Builder,
  metadata: Metadata,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Predicate {
  build_definition: BuildDefinition,
  run_details: RunDetails,
}

impl Predicate {
  pub fn new_github_actions() -> Self {
    let repo =
      std::env::var("GITHUB_REPOSITORY").expect("GITHUB_REPOSITORY not set");
    let rel_ref = std::env::var("GITHUB_WORKFLOW_REF")
      .unwrap_or_default()
      .replace(&format!("{}/", &repo), "");

    let delimn = rel_ref.find('@').unwrap();
    let (workflow_path, mut workflow_ref) = rel_ref.split_at(delimn);
    workflow_ref = &workflow_ref[1..];

    let server_url = std::env::var("GITHUB_SERVER_URL").unwrap();

    Self {
      build_definition: BuildDefinition {
        build_type: GITHUB_BUILD_TYPE,
        external_parameters: ExternalParameters {
          workflow: GhaWorkflow {
            ref_: workflow_ref.to_string(),
            repository: format!("{}/{}", server_url, &repo),
            path: workflow_path.to_string(),
          },
        },
        internal_parameters: InternalParameters {
          github: GithubInternalParameters {
            event_name: std::env::var("GITHUB_EVENT_NAME").unwrap_or_default(),
            repository_id: std::env::var("GITHUB_REPOSITORY_ID")
              .unwrap_or_default(),
            repository_owner_id: std::env::var("GITHUB_REPOSITORY_OWNER_ID")
              .unwrap_or_default(),
          },
        },
        resolved_dependencies: [ResourceDescriptor {
          uri: format!(
            "git+{}/{}@{}",
            server_url,
            &repo,
            std::env::var("GITHUB_REF").unwrap()
          ),
          digest: Some(GhaResourceDigest {
            git_commit: std::env::var("GITHUB_SHA").unwrap(),
          }),
        }],
      },
      run_details: RunDetails {
        builder: Builder {
          id: format!(
            "{}/{}",
            &GITHUB_BUILDER_ID_PREFIX,
            std::env::var("RUNNER_ENVIRONMENT").unwrap()
          ),
        },
        metadata: Metadata {
          invocation_id: format!(
            "{}/{}/actions/runs/{}/attempts/{}",
            server_url,
            repo,
            std::env::var("GITHUB_RUN_ID").unwrap(),
            std::env::var("GITHUB_RUN_ATTEMPT").unwrap()
          ),
        },
      },
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProvenanceAttestation {
  #[serde(rename = "type")]
  _type: &'static str,
  subject: Vec<Subject>,
  predicate_type: &'static str,
  predicate: Predicate,
}

impl ProvenanceAttestation {
  pub fn new_github_actions(subjects: Vec<Subject>) -> Self {
    Self {
      _type: INTOTO_STATEMENT_TYPE,
      subject: subjects,
      predicate_type: SLSA_PREDICATE_TYPE,
      predicate: Predicate::new_github_actions(),
    }
  }
}

const INTOTO_STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";
const SLSA_PREDICATE_TYPE: &str = "https://slsa.dev/provenance/v1";
const INTOTO_PAYLOAD_TYPE: &str = "application/vnd.in-toto+json";

const GITHUB_BUILDER_ID_PREFIX: &str = "https://github.com/actions/runner";
const GITHUB_BUILD_TYPE: &str =
  "https://slsa-framework.github.io/github-actions-buildtypes/workflow/v1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct X509Certificate {
  pub raw_bytes: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct X509CertificateChain {
  pub certificates: [X509Certificate; 1],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationMaterialContent {
  #[serde(rename = "$case")]
  pub case: &'static str,
  pub x509_certificate_chain: X509CertificateChain,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TlogEntry {
  pub log_index: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationMaterial {
  pub content: VerificationMaterialContent,
  pub tlog_entries: [TlogEntry; 1],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceBundle {
  pub media_type: &'static str,
  pub content: SignatureBundle,
  pub verification_material: VerificationMaterial,
}

pub async fn generate_provenance(
  http_client: &HttpClient,
  subjects: Vec<Subject>,
) -> Result<ProvenanceBundle, AnyError> {
  if !is_gha() {
    bail!("Automatic provenance is only available in GitHub Actions");
  }

  if gha_oidc_token().is_none() {
    bail!(
      "Provenance generation in Github Actions requires 'id-token' permission"
    );
  };

  let slsa = ProvenanceAttestation::new_github_actions(subjects);

  let attestation = serde_json::to_string(&slsa)?;
  let bundle = attest(http_client, &attestation, INTOTO_PAYLOAD_TYPE).await?;

  Ok(bundle)
}

pub async fn attest(
  http_client: &HttpClient,
  data: &str,
  type_: &str,
) -> Result<ProvenanceBundle, AnyError> {
  // DSSE Pre-Auth Encoding (PAE) payload
  let pae = pre_auth_encoding(type_, data);

  let signer = FulcioSigner::new(http_client)?;
  let (signature, key_material) = signer.sign(&pae).await?;

  let content = SignatureBundle {
    case: "dsseSignature",
    dsse_envelope: Envelope {
      payload_type: type_.to_string(),
      payload: BASE64_STANDARD.encode(data),
      signatures: vec![Signature {
        keyid: "",
        sig: BASE64_STANDARD.encode(signature.as_ref()),
      }],
    },
  };
  let transparency_logs =
    testify(http_client, &content, &key_material.certificate).await?;

  // First log entry is the one we're interested in
  let (_, log_entry) = transparency_logs.iter().next().unwrap();

  let bundle = ProvenanceBundle {
    media_type: "application/vnd.in-toto+json",
    content,
    verification_material: VerificationMaterial {
      content: VerificationMaterialContent {
        case: "x509CertificateChain",
        x509_certificate_chain: X509CertificateChain {
          certificates: [X509Certificate {
            raw_bytes: key_material.certificate,
          }],
        },
      },
      tlog_entries: [TlogEntry {
        log_index: log_entry.log_index,
      }],
    },
  };

  Ok(bundle)
}

static DEFAULT_FULCIO_URL: Lazy<String> = Lazy::new(|| {
  env::var("FULCIO_URL")
    .unwrap_or_else(|_| "https://fulcio.sigstore.dev".to_string())
});

static ALGORITHM: &ring::signature::EcdsaSigningAlgorithm =
  &ring::signature::ECDSA_P256_SHA256_ASN1_SIGNING;

struct KeyMaterial {
  pub _case: &'static str,
  pub certificate: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicKey {
  algorithm: &'static str,
  content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicKeyRequest {
  public_key: PublicKey,
  proof_of_possession: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Credentials {
  oidc_identity_token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateSigningCertificateRequest {
  credentials: Credentials,
  public_key_request: PublicKeyRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CertificateChain {
  certificates: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignedCertificate {
  chain: CertificateChain,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SigningCertificateResponse {
  signed_certificate_embedded_sct: Option<SignedCertificate>,
  signed_certificate_detached_sct: Option<SignedCertificate>,
}

struct FulcioSigner<'a> {
  // The ephemeral key pair used to sign.
  ephemeral_signer: EcdsaKeyPair,
  rng: SystemRandom,
  http_client: &'a HttpClient,
}

impl<'a> FulcioSigner<'a> {
  pub fn new(http_client: &'a HttpClient) -> Result<Self, AnyError> {
    let rng = SystemRandom::new();
    let document = EcdsaKeyPair::generate_pkcs8(ALGORITHM, &rng)?;
    let ephemeral_signer =
      EcdsaKeyPair::from_pkcs8(ALGORITHM, document.as_ref(), &rng)?;

    Ok(Self {
      ephemeral_signer,
      rng,
      http_client,
    })
  }

  pub async fn sign(
    self,
    data: &[u8],
  ) -> Result<(ring::signature::Signature, KeyMaterial), AnyError> {
    // Request token from GitHub Actions for audience "sigstore"
    let token = self.gha_request_token("sigstore").await?;
    // Extract the subject from the token
    let subject = extract_jwt_subject(&token)?;

    // Sign the subject to create a challenge
    let challenge =
      self.ephemeral_signer.sign(&self.rng, subject.as_bytes())?;

    let subject_public_key = self.ephemeral_signer.public_key().as_ref();
    let algorithm = spki::AlgorithmIdentifier {
      oid: elliptic_curve::ALGORITHM_OID,
      parameters: Some((&p256::NistP256::OID).into()),
    };
    let spki = spki::SubjectPublicKeyInfoRef {
      algorithm,
      subject_public_key: asn1::BitStringRef::from_bytes(subject_public_key)?,
    };
    let pem = spki.to_pem(LineEnding::LF)?;

    // Create signing certificate
    let certificates = self
      .create_signing_certificate(&token, pem, challenge)
      .await?;

    let signature = self.ephemeral_signer.sign(&self.rng, data)?;

    Ok((
      signature,
      KeyMaterial {
        _case: "x509Certificate",
        certificate: certificates[0].clone(),
      },
    ))
  }

  async fn create_signing_certificate(
    &self,
    token: &str,
    public_key: String,
    challenge: ring::signature::Signature,
  ) -> Result<Vec<String>, AnyError> {
    let url = format!("{}/api/v2/signingCert", *DEFAULT_FULCIO_URL);
    let request_body = CreateSigningCertificateRequest {
      credentials: Credentials {
        oidc_identity_token: token.to_string(),
      },
      public_key_request: PublicKeyRequest {
        public_key: PublicKey {
          algorithm: "ECDSA",
          content: public_key,
        },
        proof_of_possession: BASE64_STANDARD.encode(challenge.as_ref()),
      },
    };

    let response = self
      .http_client
      .post_json(url.parse()?, &request_body)?
      .send()
      .await?;

    let body: SigningCertificateResponse =
      http_util::body_to_json(response).await?;

    let key = body
      .signed_certificate_embedded_sct
      .or(body.signed_certificate_detached_sct)
      .ok_or_else(|| anyhow::anyhow!("No certificate chain returned"))?;
    Ok(key.chain.certificates)
  }

  async fn gha_request_token(&self, aud: &str) -> Result<String, AnyError> {
    let Ok(req_url) = env::var("ACTIONS_ID_TOKEN_REQUEST_URL") else {
      bail!("Not running in GitHub Actions");
    };

    let Some(token) = gha_oidc_token() else {
      bail!("No OIDC token available");
    };

    let mut url = req_url.parse::<Url>()?;
    url.query_pairs_mut().append_pair("audience", aud);
    let res_bytes = self
      .http_client
      .get(url)?
      .header(
        http::header::AUTHORIZATION,
        format!("Bearer {}", token)
          .parse()
          .map_err(http::Error::from)?,
      )
      .send()
      .await?
      .collect()
      .await?
      .to_bytes();
    let res: OidcTokenResponse = serde_json::from_slice(&res_bytes)?;
    Ok(res.value)
  }
}

#[derive(Deserialize)]
struct JwtSubject<'a> {
  email: Option<String>,
  sub: String,
  iss: &'a str,
}

fn extract_jwt_subject(token: &str) -> Result<String, AnyError> {
  let parts: Vec<&str> = token.split('.').collect();

  let payload = parts[1];
  let payload = STANDARD_NO_PAD.decode(payload)?;

  let subject: JwtSubject = serde_json::from_slice(&payload)?;
  match subject.iss {
    "https://accounts.google.com" | "https://oauth2.sigstore.dev/auth" => {
      Ok(subject.email.unwrap_or(subject.sub))
    }
    _ => Ok(subject.sub),
  }
}

static DEFAULT_REKOR_URL: Lazy<String> = Lazy::new(|| {
  env::var("REKOR_URL")
    .unwrap_or_else(|_| "https://rekor.sigstore.dev".to_string())
});

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
  #[allow(dead_code)]
  #[serde(rename = "logID")]
  pub log_id: String,
  pub log_index: u64,
}

type RekorEntry = HashMap<String, LogEntry>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RekorSignature {
  sig: String,
  // `publicKey` is not the standard part of
  // DSSE, but it's required by Rekor.
  public_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DsseEnvelope {
  payload: String,
  payload_type: String,
  signatures: [RekorSignature; 1],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProposedIntotoEntry {
  api_version: &'static str,
  kind: &'static str,
  spec: ProposedIntotoEntrySpec,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProposedIntotoEntrySpec {
  content: ProposedIntotoEntryContent,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProposedIntotoEntryContent {
  envelope: DsseEnvelope,
  hash: ProposedIntotoEntryHash,
  payload_hash: ProposedIntotoEntryHash,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProposedIntotoEntryHash {
  algorithm: &'static str,
  value: String,
}

// Rekor witness
async fn testify(
  http_client: &HttpClient,
  content: &SignatureBundle,
  public_key: &str,
) -> Result<RekorEntry, AnyError> {
  // Rekor "intoto" entry for the given DSSE envelope and signature.
  //
  // Calculate the value for the payloadHash field into the Rekor entry
  let payload_hash = faster_hex::hex_string(&sha2::Sha256::digest(
    content.dsse_envelope.payload.as_bytes(),
  ));

  // Calculate the value for the hash field into the Rekor entry
  let envelope_hash = faster_hex::hex_string(&{
    let dsse = DsseEnvelope {
      payload: content.dsse_envelope.payload.clone(),
      payload_type: content.dsse_envelope.payload_type.clone(),
      signatures: [RekorSignature {
        sig: content.dsse_envelope.signatures[0].sig.clone(),
        public_key: public_key.to_string(),
      }],
    };

    sha2::Sha256::digest(serde_json::to_string(&dsse)?.as_bytes())
  });

  // Re-create the DSSE envelop. `publicKey` is not the standard part of
  // DSSE, but it's required by Rekor.
  //
  // Double-encode payload and signature cause that's what Rekor expects
  let dsse = DsseEnvelope {
    payload_type: content.dsse_envelope.payload_type.clone(),
    payload: BASE64_STANDARD.encode(content.dsse_envelope.payload.clone()),
    signatures: [RekorSignature {
      sig: BASE64_STANDARD
        .encode(content.dsse_envelope.signatures[0].sig.clone()),
      public_key: BASE64_STANDARD.encode(public_key),
    }],
  };

  let proposed_intoto_entry = ProposedIntotoEntry {
    api_version: "0.0.2",
    kind: "intoto",
    spec: ProposedIntotoEntrySpec {
      content: ProposedIntotoEntryContent {
        envelope: dsse,
        hash: ProposedIntotoEntryHash {
          algorithm: "sha256",
          value: envelope_hash,
        },
        payload_hash: ProposedIntotoEntryHash {
          algorithm: "sha256",
          value: payload_hash,
        },
      },
    },
  };

  let url = format!("{}/api/v1/log/entries", *DEFAULT_REKOR_URL);
  let res = http_client
    .post_json(url.parse()?, &proposed_intoto_entry)?
    .send()
    .await?;
  let body: RekorEntry = http_util::body_to_json(res).await?;

  Ok(body)
}

#[cfg(test)]
mod tests {
  use super::ProvenanceAttestation;
  use super::Subject;
  use super::SubjectDigest;
  use std::env;

  #[test]
  fn slsa_github_actions() {
    // Set environment variable
    if env::var("GITHUB_ACTIONS").is_err() {
      env::set_var("CI", "true");
      env::set_var("GITHUB_ACTIONS", "true");
      env::set_var("ACTIONS_ID_TOKEN_REQUEST_URL", "https://example.com");
      env::set_var("ACTIONS_ID_TOKEN_REQUEST_TOKEN", "dummy");
      env::set_var("GITHUB_REPOSITORY", "littledivy/deno_sdl2");
      env::set_var("GITHUB_SERVER_URL", "https://github.com");
      env::set_var("GITHUB_REF", "refs/tags/sdl2@0.0.1");
      env::set_var("GITHUB_SHA", "lol");
      env::set_var("GITHUB_RUN_ID", "1");
      env::set_var("GITHUB_RUN_ATTEMPT", "1");
      env::set_var("RUNNER_ENVIRONMENT", "github-hosted");
      env::set_var(
        "GITHUB_WORKFLOW_REF",
        "littledivy/deno_sdl2@refs/tags/sdl2@0.0.1",
      );
    }

    let subject = Subject {
      name: "jsr:@divy/sdl2@0.0.1".to_string(),
      digest: SubjectDigest {
        sha256: "yourmom".to_string(),
      },
    };
    let slsa = ProvenanceAttestation::new_github_actions(vec![subject]);
    assert_eq!(
      slsa.subject.len(),
      1,
      "Subject should be an array per the in-toto specification"
    );
    assert_eq!(slsa.subject[0].name, "jsr:@divy/sdl2@0.0.1");
    assert_eq!(slsa.subject[0].digest.sha256, "yourmom");
  }
}
