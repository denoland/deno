// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

//! There are many types of errors in Deno:
//! - AnyError: a generic wrapper that can encapsulate any type of error.
//! - JsError: a container for the error message and stack trace for exceptions
//!   thrown in JavaScript code. We use this to pretty-print stack traces.
//! - Diagnostic: these are errors that originate in TypeScript's compiler.
//!   They're similar to JsError, in that they have line numbers. But
//!   Diagnostics are compile-time type errors, whereas JsErrors are runtime
//!   exceptions.

use crate::ops::fs_events::FsEventsError;
use crate::ops::http::HttpStartError;
use crate::ops::os::OsError;
use crate::ops::process::ProcessError;
use crate::ops::signal::SignalError;
use crate::ops::tty::TtyError;
use crate::ops::web_worker::SyncFetchError;
use crate::ops::worker_host::CreateWorkerError;
use deno_broadcast_channel::BroadcastChannelError;
use deno_cache::CacheError;
use deno_canvas::CanvasError;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url;
use deno_core::ModuleResolutionError;
use deno_cron::CronError;
use deno_crypto::DecryptError;
use deno_crypto::EncryptError;
use deno_crypto::ExportKeyError;
use deno_crypto::GenerateKeyError;
use deno_crypto::ImportKeyError;
use deno_fetch::FetchError;
use deno_fetch::HttpClientCreateError;
use deno_ffi::CallError;
use deno_ffi::CallbackError;
use deno_ffi::DlfcnError;
use deno_ffi::IRError;
use deno_ffi::ReprError;
use deno_ffi::StaticError;
use deno_fs::FsOpsError;
use deno_http::HttpError;
use deno_http::HttpNextError;
use deno_http::WebSocketUpgradeError;
use deno_io::fs::FsError;
use deno_kv::KvCheckError;
use deno_kv::KvError;
use deno_kv::KvMutationError;
use deno_napi::NApiError;
use deno_net::ops::NetError;
use deno_tls::TlsError;
use deno_web::BlobError;
use deno_web::CompressionError;
use deno_web::MessagePortError;
use deno_web::StreamResourceError;
use deno_web::WebError;
use deno_websocket::HandshakeError;
use deno_websocket::WebsocketError;
use deno_webstorage::WebStorageError;
use rustyline::error::ReadlineError;
use std::env;
use std::error::Error;
use std::io;
use std::sync::Arc;

fn get_dlopen_error_class(error: &dlopen2::Error) -> &'static str {
  use dlopen2::Error::*;
  match error {
    NullCharacter(_) => "InvalidData",
    OpeningLibraryError(ref e) => get_io_error_class(e),
    SymbolGettingError(ref e) => get_io_error_class(e),
    AddrNotMatchingDll(ref e) => get_io_error_class(e),
    NullSymbol => "NotFound",
  }
}

fn get_env_var_error_class(error: &env::VarError) -> &'static str {
  use env::VarError::*;
  match error {
    NotPresent => "NotFound",
    NotUnicode(..) => "InvalidData",
  }
}

fn get_io_error_class(error: &io::Error) -> &'static str {
  use io::ErrorKind::*;
  match error.kind() {
    NotFound => "NotFound",
    PermissionDenied => "PermissionDenied",
    ConnectionRefused => "ConnectionRefused",
    ConnectionReset => "ConnectionReset",
    ConnectionAborted => "ConnectionAborted",
    NotConnected => "NotConnected",
    AddrInUse => "AddrInUse",
    AddrNotAvailable => "AddrNotAvailable",
    BrokenPipe => "BrokenPipe",
    AlreadyExists => "AlreadyExists",
    InvalidInput => "TypeError",
    InvalidData => "InvalidData",
    TimedOut => "TimedOut",
    Interrupted => "Interrupted",
    WriteZero => "WriteZero",
    UnexpectedEof => "UnexpectedEof",
    Other => "Error",
    WouldBlock => "WouldBlock",
    // Non-exhaustive enum - might add new variants
    // in the future
    kind => {
      let kind_str = kind.to_string();
      match kind_str.as_str() {
        "FilesystemLoop" => "FilesystemLoop",
        "IsADirectory" => "IsADirectory",
        "NetworkUnreachable" => "NetworkUnreachable",
        "NotADirectory" => "NotADirectory",
        _ => "Error",
      }
    }
  }
}

fn get_module_resolution_error_class(
  _: &ModuleResolutionError,
) -> &'static str {
  "URIError"
}

fn get_notify_error_class(error: &notify::Error) -> &'static str {
  use notify::ErrorKind::*;
  match error.kind {
    Generic(_) => "Error",
    Io(ref e) => get_io_error_class(e),
    PathNotFound => "NotFound",
    WatchNotFound => "NotFound",
    InvalidConfig(_) => "InvalidData",
    MaxFilesWatch => "Error",
  }
}

fn get_regex_error_class(error: &regex::Error) -> &'static str {
  use regex::Error::*;
  match error {
    Syntax(_) => "SyntaxError",
    CompiledTooBig(_) => "RangeError",
    _ => "Error",
  }
}

fn get_serde_json_error_class(
  error: &serde_json::error::Error,
) -> &'static str {
  use deno_core::serde_json::error::*;
  match error.classify() {
    Category::Io => error
      .source()
      .and_then(|e| e.downcast_ref::<io::Error>())
      .map(get_io_error_class)
      .unwrap(),
    Category::Syntax => "SyntaxError",
    Category::Data => "InvalidData",
    Category::Eof => "UnexpectedEof",
  }
}

fn get_url_parse_error_class(_error: &url::ParseError) -> &'static str {
  "URIError"
}

fn get_hyper_error_class(_error: &hyper::Error) -> &'static str {
  "Http"
}

fn get_hyper_util_error_class(
  _error: &hyper_util::client::legacy::Error,
) -> &'static str {
  "Http"
}

fn get_hyper_v014_error_class(_error: &hyper_v014::Error) -> &'static str {
  "Http"
}

#[cfg(unix)]
pub fn get_nix_error_class(error: &nix::Error) -> &'static str {
  match error {
    nix::Error::ECHILD => "NotFound",
    nix::Error::EINVAL => "TypeError",
    nix::Error::ENOENT => "NotFound",
    nix::Error::ENOTTY => "BadResource",
    nix::Error::EPERM => "PermissionDenied",
    nix::Error::ESRCH => "NotFound",
    nix::Error::ELOOP => "FilesystemLoop",
    nix::Error::ENOTDIR => "NotADirectory",
    nix::Error::ENETUNREACH => "NetworkUnreachable",
    nix::Error::EISDIR => "IsADirectory",
    nix::Error::UnknownErrno => "Error",
    &nix::Error::ENOTSUP => unreachable!(),
    _ => "Error",
  }
}

fn get_webgpu_error_class(e: &deno_webgpu::InitError) -> &'static str {
  match e {
    deno_webgpu::InitError::Resource(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    deno_webgpu::InitError::InvalidAdapter(_) => "Error",
    deno_webgpu::InitError::RequestDevice(_) => "DOMExceptionOperationError",
    deno_webgpu::InitError::InvalidDevice(_) => "Error",
  }
}

fn get_webgpu_buffer_error_class(
  e: &deno_webgpu::buffer::BufferError,
) -> &'static str {
  match e {
    deno_webgpu::buffer::BufferError::Resource(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    deno_webgpu::buffer::BufferError::InvalidUsage => "TypeError",
    deno_webgpu::buffer::BufferError::Access(_) => "DOMExceptionOperationError",
  }
}

fn get_webgpu_bundle_error_class(
  e: &deno_webgpu::bundle::BundleError,
) -> &'static str {
  match e {
    deno_webgpu::bundle::BundleError::Resource(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    deno_webgpu::bundle::BundleError::InvalidSize => "TypeError",
  }
}

fn get_webgpu_byow_error_class(
  e: &deno_webgpu::byow::ByowError,
) -> &'static str {
  match e {
    deno_webgpu::byow::ByowError::WebGPUNotInitiated => "TypeError",
    deno_webgpu::byow::ByowError::InvalidParameters => "TypeError",
    deno_webgpu::byow::ByowError::CreateSurface(_) => "Error",
    deno_webgpu::byow::ByowError::InvalidSystem => "TypeError",
    #[cfg(any(
      target_os = "windows",
      target_os = "linux",
      target_os = "freebsd",
      target_os = "openbsd"
    ))]
    deno_webgpu::byow::ByowError::NullWindow => "TypeError",
    #[cfg(any(
      target_os = "linux",
      target_os = "freebsd",
      target_os = "openbsd"
    ))]
    deno_webgpu::byow::ByowError::NullDisplay => "TypeError",
    #[cfg(target_os = "macos")]
    deno_webgpu::byow::ByowError::NSViewDisplay => "TypeError",
  }
}

fn get_webgpu_render_pass_error_class(
  e: &deno_webgpu::render_pass::RenderPassError,
) -> &'static str {
  match e {
    deno_webgpu::render_pass::RenderPassError::Resource(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    deno_webgpu::render_pass::RenderPassError::InvalidSize => "TypeError",
  }
}

fn get_webgpu_surface_error_class(
  e: &deno_webgpu::surface::SurfaceError,
) -> &'static str {
  match e {
    deno_webgpu::surface::SurfaceError::Resource(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    deno_webgpu::surface::SurfaceError::Surface(_) => "Error",
    deno_webgpu::surface::SurfaceError::InvalidStatus => "Error",
  }
}

fn get_crypto_decrypt_error_class(e: &DecryptError) -> &'static str {
  match e {
    DecryptError::General(e) => get_crypto_shared_error_class(e),
    DecryptError::Pkcs1(_) => "Error",
    DecryptError::Failed => "DOMExceptionOperationError",
    DecryptError::InvalidLength => "TypeError",
    DecryptError::InvalidCounterLength => "TypeError",
    DecryptError::InvalidTagLength => "TypeError",
    DecryptError::InvalidKeyOrIv => "DOMExceptionOperationError",
    DecryptError::TooMuchData => "DOMExceptionOperationError",
    DecryptError::InvalidIvLength => "TypeError",
    DecryptError::Rsa(_) => "DOMExceptionOperationError",
  }
}

fn get_crypto_encrypt_error_class(e: &EncryptError) -> &'static str {
  match e {
    EncryptError::General(e) => get_crypto_shared_error_class(e),
    EncryptError::InvalidKeyOrIv => "DOMExceptionOperationError",
    EncryptError::Failed => "DOMExceptionOperationError",
    EncryptError::InvalidLength => "TypeError",
    EncryptError::InvalidIvLength => "TypeError",
    EncryptError::InvalidCounterLength => "TypeError",
    EncryptError::TooMuchData => "DOMExceptionOperationError",
  }
}

fn get_crypto_shared_error_class(e: &deno_crypto::SharedError) -> &'static str {
  match e {
    deno_crypto::SharedError::ExpectedValidPrivateKey => "TypeError",
    deno_crypto::SharedError::ExpectedValidPublicKey => "TypeError",
    deno_crypto::SharedError::ExpectedValidPrivateECKey => "TypeError",
    deno_crypto::SharedError::ExpectedValidPublicECKey => "TypeError",
    deno_crypto::SharedError::ExpectedPrivateKey => "TypeError",
    deno_crypto::SharedError::ExpectedPublicKey => "TypeError",
    deno_crypto::SharedError::ExpectedSecretKey => "TypeError",
    deno_crypto::SharedError::FailedDecodePrivateKey => {
      "DOMExceptionOperationError"
    }
    deno_crypto::SharedError::FailedDecodePublicKey => {
      "DOMExceptionOperationError"
    }
    deno_crypto::SharedError::UnsupportedFormat => {
      "DOMExceptionNotSupportedError"
    }
  }
}

fn get_crypto_ed25519_error_class(
  e: &deno_crypto::Ed25519Error,
) -> &'static str {
  match e {
    deno_crypto::Ed25519Error::FailedExport => "DOMExceptionOperationError",
    deno_crypto::Ed25519Error::Der(_) => "Error",
    deno_crypto::Ed25519Error::KeyRejected(_) => "Error",
  }
}

fn get_crypto_export_key_error_class(e: &ExportKeyError) -> &'static str {
  match e {
    ExportKeyError::General(e) => get_crypto_shared_error_class(e),
    ExportKeyError::Der(_) => "Error",
    ExportKeyError::UnsupportedNamedCurve => "DOMExceptionNotSupportedError",
  }
}

fn get_crypto_generate_key_error_class(e: &GenerateKeyError) -> &'static str {
  match e {
    GenerateKeyError::General(e) => get_crypto_shared_error_class(e),
    GenerateKeyError::BadPublicExponent => "DOMExceptionOperationError",
    GenerateKeyError::InvalidHMACKeyLength => "DOMExceptionOperationError",
    GenerateKeyError::FailedRSAKeySerialization => "DOMExceptionOperationError",
    GenerateKeyError::InvalidAESKeyLength => "DOMExceptionOperationError",
    GenerateKeyError::FailedRSAKeyGeneration => "DOMExceptionOperationError",
    GenerateKeyError::FailedECKeyGeneration => "DOMExceptionOperationError",
    GenerateKeyError::FailedKeyGeneration => "DOMExceptionOperationError",
  }
}

fn get_crypto_import_key_error_class(e: &ImportKeyError) -> &'static str {
  match e {
    ImportKeyError::General(e) => get_crypto_shared_error_class(e),
    ImportKeyError::InvalidModulus => "DOMExceptionDataError",
    ImportKeyError::InvalidPublicExponent => "DOMExceptionDataError",
    ImportKeyError::InvalidPrivateExponent => "DOMExceptionDataError",
    ImportKeyError::InvalidFirstPrimeFactor => "DOMExceptionDataError",
    ImportKeyError::InvalidSecondPrimeFactor => "DOMExceptionDataError",
    ImportKeyError::InvalidFirstCRTExponent => "DOMExceptionDataError",
    ImportKeyError::InvalidSecondCRTExponent => "DOMExceptionDataError",
    ImportKeyError::InvalidCRTCoefficient => "DOMExceptionDataError",
    ImportKeyError::InvalidB64Coordinate => "DOMExceptionDataError",
    ImportKeyError::InvalidRSAPublicKey => "DOMExceptionDataError",
    ImportKeyError::InvalidRSAPrivateKey => "DOMExceptionDataError",
    ImportKeyError::UnsupportedAlgorithm => "DOMExceptionDataError",
    ImportKeyError::PublicKeyTooLong => "DOMExceptionDataError",
    ImportKeyError::PrivateKeyTooLong => "DOMExceptionDataError",
    ImportKeyError::InvalidP256ECPoint => "DOMExceptionDataError",
    ImportKeyError::InvalidP384ECPoint => "DOMExceptionDataError",
    ImportKeyError::InvalidP521ECPoint => "DOMExceptionDataError",
    ImportKeyError::UnsupportedNamedCurve => "DOMExceptionDataError",
    ImportKeyError::CurveMismatch => "DOMExceptionDataError",
    ImportKeyError::InvalidKeyData => "DOMExceptionDataError",
    ImportKeyError::InvalidJWKPrivateKey => "DOMExceptionDataError",
    ImportKeyError::EllipticCurve(_) => "DOMExceptionDataError",
    ImportKeyError::ExpectedValidPkcs8Data => "DOMExceptionDataError",
    ImportKeyError::MalformedParameters => "DOMExceptionDataError",
    ImportKeyError::Spki(_) => "DOMExceptionDataError",
    ImportKeyError::InvalidP256ECSPKIData => "DOMExceptionDataError",
    ImportKeyError::InvalidP384ECSPKIData => "DOMExceptionDataError",
    ImportKeyError::InvalidP521ECSPKIData => "DOMExceptionDataError",
    ImportKeyError::Der(_) => "DOMExceptionDataError",
  }
}

fn get_crypto_x448_error_class(e: &deno_crypto::X448Error) -> &'static str {
  match e {
    deno_crypto::X448Error::FailedExport => "DOMExceptionOperationError",
    deno_crypto::X448Error::Der(_) => "Error",
  }
}

fn get_crypto_x25519_error_class(e: &deno_crypto::X25519Error) -> &'static str {
  match e {
    deno_crypto::X25519Error::FailedExport => "DOMExceptionOperationError",
    deno_crypto::X25519Error::Der(_) => "Error",
  }
}

fn get_crypto_error_class(e: &deno_crypto::Error) -> &'static str {
  match e {
    deno_crypto::Error::Der(_) => "Error",
    deno_crypto::Error::JoinError(_) => "Error",
    deno_crypto::Error::MissingArgumentHash => "TypeError",
    deno_crypto::Error::MissingArgumentSaltLength => "TypeError",
    deno_crypto::Error::Other(e) => get_error_class_name(e).unwrap_or("Error"),
    deno_crypto::Error::UnsupportedAlgorithm => "TypeError",
    deno_crypto::Error::KeyRejected(_) => "Error",
    deno_crypto::Error::RSA(_) => "Error",
    deno_crypto::Error::Pkcs1(_) => "Error",
    deno_crypto::Error::Unspecified(_) => "Error",
    deno_crypto::Error::InvalidKeyFormat => "TypeError",
    deno_crypto::Error::MissingArgumentPublicKey => "TypeError",
    deno_crypto::Error::P256Ecdsa(_) => "Error",
    deno_crypto::Error::DecodePrivateKey => "TypeError",
    deno_crypto::Error::MissingArgumentNamedCurve => "TypeError",
    deno_crypto::Error::MissingArgumentInfo => "TypeError",
    deno_crypto::Error::HKDFLengthTooLarge => "DOMExceptionOperationError",
    deno_crypto::Error::General(e) => get_crypto_shared_error_class(e),
    deno_crypto::Error::Base64Decode(_) => "Error",
    deno_crypto::Error::DataInvalidSize => "TypeError",
    deno_crypto::Error::InvalidKeyLength => "TypeError",
    deno_crypto::Error::EncryptionError => "DOMExceptionOperationError",
    deno_crypto::Error::DecryptionError => "DOMExceptionOperationError",
    deno_crypto::Error::ArrayBufferViewLengthExceeded(_) => {
      "DOMExceptionQuotaExceededError"
    }
  }
}

fn get_napi_error_class(e: &NApiError) -> &'static str {
  match e {
    NApiError::InvalidPath
    | NApiError::LibLoading(_)
    | NApiError::ModuleNotFound(_) => "TypeError",
    NApiError::Permission(e) => get_error_class_name(e).unwrap_or("Error"),
  }
}

fn get_web_error_class(e: &WebError) -> &'static str {
  match e {
    WebError::Base64Decode => "DOMExceptionInvalidCharacterError",
    WebError::InvalidEncodingLabel(_) => "RangeError",
    WebError::BufferTooLong => "TypeError",
    WebError::ValueTooLarge => "RangeError",
    WebError::BufferTooSmall => "RangeError",
    WebError::DataInvalid => "TypeError",
    WebError::DataError(_) => "Error",
  }
}

fn get_web_compression_error_class(e: &CompressionError) -> &'static str {
  match e {
    CompressionError::UnsupportedFormat => "TypeError",
    CompressionError::ResourceClosed => "TypeError",
    CompressionError::IoTypeError(_) => "TypeError",
    CompressionError::Io(e) => get_io_error_class(e),
  }
}

fn get_web_message_port_error_class(e: &MessagePortError) -> &'static str {
  match e {
    MessagePortError::InvalidTransfer => "TypeError",
    MessagePortError::NotReady => "TypeError",
    MessagePortError::TransferSelf => "TypeError",
    MessagePortError::Canceled(e) => {
      let io_err: io::Error = e.to_owned().into();
      get_io_error_class(&io_err)
    }
    MessagePortError::Resource(e) => get_error_class_name(e).unwrap_or("Error"),
  }
}

fn get_web_stream_resource_error_class(
  e: &StreamResourceError,
) -> &'static str {
  match e {
    StreamResourceError::Canceled(e) => {
      let io_err: io::Error = e.to_owned().into();
      get_io_error_class(&io_err)
    }
    StreamResourceError::Js(_) => "TypeError",
  }
}

fn get_web_blob_error_class(e: &BlobError) -> &'static str {
  match e {
    BlobError::BlobPartNotFound => "TypeError",
    BlobError::SizeLargerThanBlobPart => "TypeError",
    BlobError::BlobURLsNotSupported => "TypeError",
    BlobError::Url(_) => "Error",
  }
}

fn get_ffi_repr_error_class(e: &ReprError) -> &'static str {
  match e {
    ReprError::InvalidOffset => "TypeError",
    ReprError::InvalidArrayBuffer => "TypeError",
    ReprError::DestinationLengthTooShort => "RangeError",
    ReprError::InvalidCString => "TypeError",
    ReprError::CStringTooLong => "TypeError",
    ReprError::InvalidBool => "TypeError",
    ReprError::InvalidU8 => "TypeError",
    ReprError::InvalidI8 => "TypeError",
    ReprError::InvalidU16 => "TypeError",
    ReprError::InvalidI16 => "TypeError",
    ReprError::InvalidU32 => "TypeError",
    ReprError::InvalidI32 => "TypeError",
    ReprError::InvalidU64 => "TypeError",
    ReprError::InvalidI64 => "TypeError",
    ReprError::InvalidF32 => "TypeError",
    ReprError::InvalidF64 => "TypeError",
    ReprError::InvalidPointer => "TypeError",
    ReprError::Permission(e) => get_error_class_name(e).unwrap_or("Error"),
  }
}

fn get_ffi_dlfcn_error_class(e: &DlfcnError) -> &'static str {
  match e {
    DlfcnError::RegisterSymbol { .. } => "Error",
    DlfcnError::Dlopen(_) => "Error",
    DlfcnError::Permission(e) => get_error_class_name(e).unwrap_or("Error"),
    DlfcnError::Other(e) => get_error_class_name(e).unwrap_or("Error"),
  }
}

fn get_ffi_static_error_class(e: &StaticError) -> &'static str {
  match e {
    StaticError::Dlfcn(e) => get_ffi_dlfcn_error_class(e),
    StaticError::InvalidTypeVoid => "TypeError",
    StaticError::InvalidTypeStruct => "TypeError",
    StaticError::Resource(e) => get_error_class_name(e).unwrap_or("Error"),
  }
}

fn get_ffi_callback_error_class(e: &CallbackError) -> &'static str {
  match e {
    CallbackError::Resource(e) => get_error_class_name(e).unwrap_or("Error"),
    CallbackError::Other(e) => get_error_class_name(e).unwrap_or("Error"),
    CallbackError::Permission(e) => get_error_class_name(e).unwrap_or("Error"),
  }
}

fn get_ffi_call_error_class(e: &CallError) -> &'static str {
  match e {
    CallError::IR(_) => "TypeError",
    CallError::NonblockingCallFailure(_) => "Error",
    CallError::InvalidSymbol(_) => "TypeError",
    CallError::Permission(e) => get_error_class_name(e).unwrap_or("Error"),
    CallError::Callback(e) => get_ffi_callback_error_class(e),
  }
}

fn get_webstorage_class_name(e: &WebStorageError) -> &'static str {
  match e {
    WebStorageError::ContextNotSupported => "DOMExceptionNotSupportedError",
    WebStorageError::Sqlite(_) => todo!(),
    WebStorageError::Io(e) => get_io_error_class(e),
    WebStorageError::StorageExceeded => "DOMExceptionQuotaExceededError",
  }
}

fn get_tls_error_class(e: &TlsError) -> &'static str {
  match e {
    TlsError::Rustls(_) => "Error",
    TlsError::UnableAddPemFileToCert(e) => get_io_error_class(e),
    TlsError::CertInvalid
    | TlsError::CertsNotFound
    | TlsError::KeysNotFound
    | TlsError::KeyDecode => "InvalidData",
  }
}

pub fn get_cron_error_class(e: &CronError) -> &'static str {
  match e {
    CronError::Resource(e) => {
      deno_core::error::get_custom_error_class(e).unwrap_or("Error")
    }
    CronError::NameExceeded(_) => "TypeError",
    CronError::NameInvalid => "TypeError",
    CronError::AlreadyExists => "TypeError",
    CronError::TooManyCrons => "TypeError",
    CronError::InvalidCron => "TypeError",
    CronError::InvalidBackoff => "TypeError",
    CronError::AcquireError(_) => "Error",
    CronError::Other(e) => get_error_class_name(e).unwrap_or("Error"),
  }
}

fn get_canvas_error(e: &CanvasError) -> &'static str {
  match e {
    CanvasError::UnsupportedColorType(_) => "TypeError",
    CanvasError::Image(_) => "Error",
  }
}

pub fn get_cache_error(error: &CacheError) -> &'static str {
  match error {
    CacheError::Sqlite(_) => "Error",
    CacheError::JoinError(_) => "Error",
    CacheError::Resource(err) => {
      deno_core::error::get_custom_error_class(err).unwrap_or("Error")
    }
    CacheError::Other(e) => get_error_class_name(e).unwrap_or("Error"),
    CacheError::Io(err) => get_io_error_class(err),
  }
}

fn get_broadcast_channel_error(error: &BroadcastChannelError) -> &'static str {
  match error {
    BroadcastChannelError::Resource(err) => {
      deno_core::error::get_custom_error_class(err).unwrap()
    }
    BroadcastChannelError::MPSCSendError(_) => "Error",
    BroadcastChannelError::BroadcastSendError(_) => "Error",
    BroadcastChannelError::Other(err) => {
      get_error_class_name(err).unwrap_or("Error")
    }
  }
}

fn get_fetch_error(error: &FetchError) -> &'static str {
  match error {
    FetchError::Resource(e) | FetchError::Permission(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    FetchError::NetworkError => "TypeError",
    FetchError::FsNotGet(_) => "TypeError",
    FetchError::InvalidUrl(_) => "TypeError",
    FetchError::InvalidHeaderName(_) => "TypeError",
    FetchError::InvalidHeaderValue(_) => "TypeError",
    FetchError::DataUrl(_) => "TypeError",
    FetchError::Base64(_) => "TypeError",
    FetchError::BlobNotFound => "TypeError",
    FetchError::SchemeNotSupported(_) => "TypeError",
    FetchError::RequestCanceled => "TypeError",
    FetchError::Http(_) => "Error",
    FetchError::ClientCreate(e) => get_http_client_create_error(e),
    FetchError::Url(e) => get_url_parse_error_class(e),
    FetchError::Method(_) => "TypeError",
    FetchError::ClientSend(_) => "TypeError",
    FetchError::RequestBuilderHook(_) => "TypeError",
    FetchError::Io(e) => get_io_error_class(e),
    FetchError::Hyper(e) => get_hyper_error_class(e),
  }
}

fn get_http_client_create_error(error: &HttpClientCreateError) -> &'static str {
  match error {
    HttpClientCreateError::Tls(_) => "TypeError",
    HttpClientCreateError::InvalidUserAgent(_) => "TypeError",
    HttpClientCreateError::InvalidProxyUrl => "TypeError",
    HttpClientCreateError::HttpVersionSelectionInvalid => "TypeError",
    HttpClientCreateError::RootCertStore(_) => "TypeError",
  }
}

fn get_websocket_error(error: &WebsocketError) -> &'static str {
  match error {
    WebsocketError::Permission(e) | WebsocketError::Resource(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    WebsocketError::Url(e) => get_url_parse_error_class(e),
    WebsocketError::Io(e) => get_io_error_class(e),
    WebsocketError::WebSocket(_) => "TypeError",
    WebsocketError::ConnectionFailed(_) => "DOMExceptionNetworkError",
    WebsocketError::Uri(_) => "Error",
    WebsocketError::Canceled(e) => {
      let io_err: io::Error = e.to_owned().into();
      get_io_error_class(&io_err)
    }
  }
}

fn get_websocket_handshake_error(error: &HandshakeError) -> &'static str {
  match error {
    HandshakeError::RootStoreError(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    HandshakeError::Tls(e) => get_tls_error_class(e),
    HandshakeError::MissingPath => "TypeError",
    HandshakeError::Http(_) => "Error",
    HandshakeError::InvalidHostname(_) => "TypeError",
    HandshakeError::Io(e) => get_io_error_class(e),
    HandshakeError::Rustls(_) => "Error",
    HandshakeError::H2(_) => "Error",
    HandshakeError::NoH2Alpn => "Error",
    HandshakeError::InvalidStatusCode(_) => "Error",
    HandshakeError::WebSocket(_) => "TypeError",
    HandshakeError::HeaderName(_) => "TypeError",
    HandshakeError::HeaderValue(_) => "TypeError",
  }
}

fn get_fs_error(error: &FsOpsError) -> &'static str {
  match error {
    FsOpsError::Io(e) => get_io_error_class(e),
    FsOpsError::OperationError(e) => match &e.err {
      FsError::Io(e) => get_io_error_class(e),
      FsError::FileBusy => "Busy",
      FsError::NotSupported => "NotSupported",
      FsError::NotCapable(_) => "NotCapable",
    },
    FsOpsError::Permission(e)
    | FsOpsError::Resource(e)
    | FsOpsError::Other(e) => get_error_class_name(e).unwrap_or("Error"),
    FsOpsError::InvalidUtf8(_) => "InvalidData",
    FsOpsError::StripPrefix(_) => "Error",
    FsOpsError::Canceled(e) => {
      let io_err: io::Error = e.to_owned().into();
      get_io_error_class(&io_err)
    }
    FsOpsError::InvalidSeekMode(_) => "TypeError",
    FsOpsError::InvalidControlCharacter(_) => "Error",
    FsOpsError::InvalidCharacter(_) => "Error",
    #[cfg(windows)]
    FsOpsError::InvalidTrailingCharacter => "Error",
    FsOpsError::NotCapableAccess { .. } => "NotCapable",
    FsOpsError::NotCapable(_) => "NotCapable",
  }
}

fn get_kv_error(error: &KvError) -> &'static str {
  match error {
    KvError::DatabaseHandler(e) | KvError::Resource(e) | KvError::Kv(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    KvError::TooManyRanges(_) => "TypeError",
    KvError::TooManyEntries(_) => "TypeError",
    KvError::TooManyChecks(_) => "TypeError",
    KvError::TooManyMutations(_) => "TypeError",
    KvError::TooManyKeys(_) => "TypeError",
    KvError::InvalidLimit => "TypeError",
    KvError::InvalidBoundaryKey => "TypeError",
    KvError::KeyTooLargeToRead(_) => "TypeError",
    KvError::KeyTooLargeToWrite(_) => "TypeError",
    KvError::TotalMutationTooLarge(_) => "TypeError",
    KvError::TotalKeyTooLarge(_) => "TypeError",
    KvError::Io(e) => get_io_error_class(e),
    KvError::QueueMessageNotFound => "TypeError",
    KvError::StartKeyNotInKeyspace => "TypeError",
    KvError::EndKeyNotInKeyspace => "TypeError",
    KvError::StartKeyGreaterThanEndKey => "TypeError",
    KvError::InvalidCheck(e) => match e {
      KvCheckError::InvalidVersionstamp => "TypeError",
      KvCheckError::Io(e) => get_io_error_class(e),
    },
    KvError::InvalidMutation(e) => match e {
      KvMutationError::BigInt(_) => "Error",
      KvMutationError::Io(e) => get_io_error_class(e),
      KvMutationError::InvalidMutationWithValue(_) => "TypeError",
      KvMutationError::InvalidMutationWithoutValue(_) => "TypeError",
    },
    KvError::InvalidEnqueue(e) => get_io_error_class(e),
    KvError::EmptyKey => "TypeError",
    KvError::ValueTooLarge(_) => "TypeError",
    KvError::EnqueuePayloadTooLarge(_) => "TypeError",
    KvError::InvalidCursor => "TypeError",
    KvError::CursorOutOfBounds => "TypeError",
    KvError::InvalidRange => "TypeError",
  }
}

fn get_net_error(error: &NetError) -> &'static str {
  match error {
    NetError::ListenerClosed => "BadResource",
    NetError::ListenerBusy => "Busy",
    NetError::SocketClosed => "BadResource",
    NetError::SocketClosedNotConnected => "NotConnected",
    NetError::SocketBusy => "Busy",
    NetError::Io(e) => get_io_error_class(e),
    NetError::AcceptTaskOngoing => "Busy",
    NetError::RootCertStore(e)
    | NetError::Permission(e)
    | NetError::Resource(e) => get_error_class_name(e).unwrap_or("Error"),
    NetError::NoResolvedAddress => "Error",
    NetError::AddrParse(_) => "Error",
    NetError::Map(e) => get_net_map_error(e),
    NetError::Canceled(e) => {
      let io_err: io::Error = e.to_owned().into();
      get_io_error_class(&io_err)
    }
    NetError::DnsNotFound(_) => "NotFound",
    NetError::DnsNotConnected(_) => "NotConnected",
    NetError::DnsTimedOut(_) => "TimedOut",
    NetError::Dns(_) => "Error",
    NetError::UnsupportedRecordType => "NotSupported",
    NetError::InvalidUtf8(_) => "InvalidData",
    NetError::UnexpectedKeyType => "Error",
    NetError::InvalidHostname(_) => "TypeError",
    NetError::TcpStreamBusy => "Busy",
    NetError::Rustls(_) => "Error",
    NetError::Tls(e) => get_tls_error_class(e),
    NetError::ListenTlsRequiresKey => "InvalidData",
    NetError::Reunite(_) => "Error",
  }
}

fn get_net_map_error(error: &deno_net::io::MapError) -> &'static str {
  match error {
    deno_net::io::MapError::Io(e) => get_io_error_class(e),
    deno_net::io::MapError::NoResources => "Error",
  }
}

fn get_create_worker_error(error: &CreateWorkerError) -> &'static str {
  match error {
    CreateWorkerError::ClassicWorkers => "DOMExceptionNotSupportedError",
    CreateWorkerError::Permission(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    CreateWorkerError::ModuleResolution(e) => {
      get_module_resolution_error_class(e)
    }
    CreateWorkerError::Io(e) => get_io_error_class(e),
    CreateWorkerError::MessagePort(e) => get_web_message_port_error_class(e),
  }
}

fn get_tty_error(error: &TtyError) -> &'static str {
  match error {
    TtyError::Resource(e) | TtyError::Other(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    TtyError::Io(e) => get_io_error_class(e),
    #[cfg(unix)]
    TtyError::Nix(e) => get_nix_error_class(e),
  }
}

fn get_readline_error(error: &ReadlineError) -> &'static str {
  match error {
    ReadlineError::Io(e) => get_io_error_class(e),
    ReadlineError::Eof => "Error",
    ReadlineError::Interrupted => "Error",
    #[cfg(unix)]
    ReadlineError::Errno(e) => get_nix_error_class(e),
    ReadlineError::WindowResized => "Error",
    #[cfg(windows)]
    ReadlineError::Decode(_) => "Error",
    #[cfg(windows)]
    ReadlineError::SystemError(_) => "Error",
    _ => "Error",
  }
}

fn get_signal_error(error: &SignalError) -> &'static str {
  match error {
    SignalError::InvalidSignalStr(_) => "TypeError",
    SignalError::InvalidSignalInt(_) => "TypeError",
    SignalError::SignalNotAllowed(_) => "TypeError",
    SignalError::Io(e) => get_io_error_class(e),
  }
}

fn get_fs_events_error(error: &FsEventsError) -> &'static str {
  match error {
    FsEventsError::Resource(e) | FsEventsError::Permission(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    FsEventsError::Notify(e) => get_notify_error_class(e),
    FsEventsError::Canceled(e) => {
      let io_err: io::Error = e.to_owned().into();
      get_io_error_class(&io_err)
    }
  }
}

fn get_http_start_error(error: &HttpStartError) -> &'static str {
  match error {
    HttpStartError::TcpStreamInUse => "Busy",
    HttpStartError::TlsStreamInUse => "Busy",
    HttpStartError::UnixSocketInUse => "Busy",
    HttpStartError::ReuniteTcp(_) => "Error",
    #[cfg(unix)]
    HttpStartError::ReuniteUnix(_) => "Error",
    HttpStartError::Io(e) => get_io_error_class(e),
    HttpStartError::Other(e) => get_error_class_name(e).unwrap_or("Error"),
  }
}

fn get_process_error(error: &ProcessError) -> &'static str {
  match error {
    ProcessError::SpawnFailed { error, .. } => get_process_error(error),
    ProcessError::FailedResolvingCwd(e) | ProcessError::Io(e) => {
      get_io_error_class(e)
    }
    ProcessError::Permission(e) | ProcessError::Resource(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
    ProcessError::BorrowMut(_) => "Error",
    ProcessError::Which(_) => "Error",
    ProcessError::ChildProcessAlreadyTerminated => "TypeError",
    ProcessError::Signal(e) => get_signal_error(e),
    ProcessError::MissingCmd => "Error",
    ProcessError::InvalidPid => "TypeError",
    #[cfg(unix)]
    ProcessError::Nix(e) => get_nix_error_class(e),
  }
}

fn get_http_error(error: &HttpError) -> &'static str {
  match error {
    HttpError::Canceled(e) => {
      let io_err: io::Error = e.to_owned().into();
      get_io_error_class(&io_err)
    }
    HttpError::HyperV014(e) => get_hyper_v014_error_class(e),
    HttpError::InvalidHeaderName(_) => "Error",
    HttpError::InvalidHeaderValue(_) => "Error",
    HttpError::Http(_) => "Error",
    HttpError::ResponseHeadersAlreadySent => "Http",
    HttpError::ConnectionClosedWhileSendingResponse => "Http",
    HttpError::AlreadyInUse => "Http",
    HttpError::Io(e) => get_io_error_class(e),
    HttpError::NoResponseHeaders => "Http",
    HttpError::ResponseAlreadyCompleted => "Http",
    HttpError::UpgradeBodyUsed => "Http",
    HttpError::Resource(e) | HttpError::Other(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
  }
}

fn get_http_next_error(error: &HttpNextError) -> &'static str {
  match error {
    HttpNextError::Io(e) => get_io_error_class(e),
    HttpNextError::WebSocketUpgrade(e) => get_websocket_upgrade_error(e),
    HttpNextError::Hyper(e) => get_hyper_error_class(e),
    HttpNextError::JoinError(_) => "Error",
    HttpNextError::Canceled(e) => {
      let io_err: io::Error = e.to_owned().into();
      get_io_error_class(&io_err)
    }
    HttpNextError::UpgradeUnavailable(_) => "Error",
    HttpNextError::HttpPropertyExtractor(e) | HttpNextError::Resource(e) => {
      get_error_class_name(e).unwrap_or("Error")
    }
  }
}

fn get_websocket_upgrade_error(error: &WebSocketUpgradeError) -> &'static str {
  match error {
    WebSocketUpgradeError::InvalidHeaders => "Http",
    WebSocketUpgradeError::HttpParse(_) => "Error",
    WebSocketUpgradeError::Http(_) => "Error",
    WebSocketUpgradeError::Utf8(_) => "Error",
    WebSocketUpgradeError::InvalidHeaderName(_) => "Error",
    WebSocketUpgradeError::InvalidHeaderValue(_) => "Error",
    WebSocketUpgradeError::InvalidHttpStatusLine => "Http",
    WebSocketUpgradeError::UpgradeBufferAlreadyCompleted => "Http",
  }
}

fn get_os_error(error: &OsError) -> &'static str {
  match error {
    OsError::Permission(e) => get_error_class_name(e).unwrap_or("Error"),
    OsError::InvalidUtf8(_) => "InvalidData",
    OsError::EnvEmptyKey => "TypeError",
    OsError::EnvInvalidKey(_) => "TypeError",
    OsError::EnvInvalidValue(_) => "TypeError",
    OsError::Io(e) => get_io_error_class(e),
    OsError::Var(e) => get_env_var_error_class(e),
  }
}

fn get_sync_fetch_error(error: &SyncFetchError) -> &'static str {
  match error {
    SyncFetchError::BlobUrlsNotSupportedInContext => "TypeError",
    SyncFetchError::Io(e) => get_io_error_class(e),
    SyncFetchError::InvalidScriptUrl => "TypeError",
    SyncFetchError::InvalidStatusCode(_) => "TypeError",
    SyncFetchError::ClassicScriptSchemeUnsupportedInWorkers(_) => "TypeError",
    SyncFetchError::InvalidUri(_) => "Error",
    SyncFetchError::InvalidMimeType(_) => "DOMExceptionNetworkError",
    SyncFetchError::MissingMimeType => "DOMExceptionNetworkError",
    SyncFetchError::Fetch(e) => get_fetch_error(e),
    SyncFetchError::Join(_) => "Error",
    SyncFetchError::Other(e) => get_error_class_name(e).unwrap_or("Error"),
  }
}

pub fn get_error_class_name(e: &AnyError) -> Option<&'static str> {
  deno_core::error::get_custom_error_class(e)
    .or_else(|| e.downcast_ref::<NApiError>().map(get_napi_error_class))
    .or_else(|| e.downcast_ref::<WebError>().map(get_web_error_class))
    .or_else(|| {
      e.downcast_ref::<CreateWorkerError>()
        .map(get_create_worker_error)
    })
    .or_else(|| e.downcast_ref::<TtyError>().map(get_tty_error))
    .or_else(|| e.downcast_ref::<ReadlineError>().map(get_readline_error))
    .or_else(|| e.downcast_ref::<SignalError>().map(get_signal_error))
    .or_else(|| e.downcast_ref::<FsEventsError>().map(get_fs_events_error))
    .or_else(|| e.downcast_ref::<HttpStartError>().map(get_http_start_error))
    .or_else(|| e.downcast_ref::<ProcessError>().map(get_process_error))
    .or_else(|| e.downcast_ref::<OsError>().map(get_os_error))
    .or_else(|| e.downcast_ref::<SyncFetchError>().map(get_sync_fetch_error))
    .or_else(|| {
      e.downcast_ref::<CompressionError>()
        .map(get_web_compression_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<MessagePortError>()
        .map(get_web_message_port_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<StreamResourceError>()
        .map(get_web_stream_resource_error_class)
    })
    .or_else(|| e.downcast_ref::<BlobError>().map(get_web_blob_error_class))
    .or_else(|| e.downcast_ref::<IRError>().map(|_| "TypeError"))
    .or_else(|| e.downcast_ref::<ReprError>().map(get_ffi_repr_error_class))
    .or_else(|| e.downcast_ref::<HttpError>().map(get_http_error))
    .or_else(|| e.downcast_ref::<HttpNextError>().map(get_http_next_error))
    .or_else(|| {
      e.downcast_ref::<WebSocketUpgradeError>()
        .map(get_websocket_upgrade_error)
    })
    .or_else(|| e.downcast_ref::<FsOpsError>().map(get_fs_error))
    .or_else(|| {
      e.downcast_ref::<DlfcnError>()
        .map(get_ffi_dlfcn_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<StaticError>()
        .map(get_ffi_static_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<CallbackError>()
        .map(get_ffi_callback_error_class)
    })
    .or_else(|| e.downcast_ref::<CallError>().map(get_ffi_call_error_class))
    .or_else(|| e.downcast_ref::<TlsError>().map(get_tls_error_class))
    .or_else(|| e.downcast_ref::<CronError>().map(get_cron_error_class))
    .or_else(|| e.downcast_ref::<CanvasError>().map(get_canvas_error))
    .or_else(|| e.downcast_ref::<CacheError>().map(get_cache_error))
    .or_else(|| e.downcast_ref::<WebsocketError>().map(get_websocket_error))
    .or_else(|| {
      e.downcast_ref::<HandshakeError>()
        .map(get_websocket_handshake_error)
    })
    .or_else(|| e.downcast_ref::<KvError>().map(get_kv_error))
    .or_else(|| e.downcast_ref::<FetchError>().map(get_fetch_error))
    .or_else(|| {
      e.downcast_ref::<HttpClientCreateError>()
        .map(get_http_client_create_error)
    })
    .or_else(|| e.downcast_ref::<NetError>().map(get_net_error))
    .or_else(|| {
      e.downcast_ref::<deno_net::io::MapError>()
        .map(get_net_map_error)
    })
    .or_else(|| {
      e.downcast_ref::<BroadcastChannelError>()
        .map(get_broadcast_channel_error)
    })
    .or_else(|| {
      e.downcast_ref::<deno_webgpu::InitError>()
        .map(get_webgpu_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_webgpu::buffer::BufferError>()
        .map(get_webgpu_buffer_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_webgpu::bundle::BundleError>()
        .map(get_webgpu_bundle_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_webgpu::byow::ByowError>()
        .map(get_webgpu_byow_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_webgpu::render_pass::RenderPassError>()
        .map(get_webgpu_render_pass_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_webgpu::surface::SurfaceError>()
        .map(get_webgpu_surface_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<DecryptError>()
        .map(get_crypto_decrypt_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<EncryptError>()
        .map(get_crypto_encrypt_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_crypto::SharedError>()
        .map(get_crypto_shared_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_crypto::Ed25519Error>()
        .map(get_crypto_ed25519_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<ExportKeyError>()
        .map(get_crypto_export_key_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<GenerateKeyError>()
        .map(get_crypto_generate_key_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<ImportKeyError>()
        .map(get_crypto_import_key_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_crypto::X448Error>()
        .map(get_crypto_x448_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_crypto::X25519Error>()
        .map(get_crypto_x25519_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_crypto::Error>()
        .map(get_crypto_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<WebStorageError>()
        .map(get_webstorage_class_name)
    })
    .or_else(|| {
      e.downcast_ref::<deno_url::UrlPatternError>()
        .map(|_| "TypeError")
    })
    .or_else(|| {
      e.downcast_ref::<dlopen2::Error>()
        .map(get_dlopen_error_class)
    })
    .or_else(|| e.downcast_ref::<hyper::Error>().map(get_hyper_error_class))
    .or_else(|| {
      e.downcast_ref::<hyper_util::client::legacy::Error>()
        .map(get_hyper_util_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<hyper_v014::Error>()
        .map(get_hyper_v014_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<Arc<hyper_v014::Error>>()
        .map(|e| get_hyper_v014_error_class(e))
    })
    .or_else(|| {
      e.downcast_ref::<deno_core::Canceled>().map(|e| {
        let io_err: io::Error = e.to_owned().into();
        get_io_error_class(&io_err)
      })
    })
    .or_else(|| {
      e.downcast_ref::<env::VarError>()
        .map(get_env_var_error_class)
    })
    .or_else(|| e.downcast_ref::<io::Error>().map(get_io_error_class))
    .or_else(|| {
      e.downcast_ref::<ModuleResolutionError>()
        .map(get_module_resolution_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<notify::Error>()
        .map(get_notify_error_class)
    })
    .or_else(|| e.downcast_ref::<regex::Error>().map(get_regex_error_class))
    .or_else(|| {
      e.downcast_ref::<serde_json::error::Error>()
        .map(get_serde_json_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<url::ParseError>()
        .map(get_url_parse_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<deno_kv::sqlite::SqliteBackendError>()
        .map(|_| "TypeError")
    })
    .or_else(|| {
      #[cfg(unix)]
      let maybe_get_nix_error_class =
        || e.downcast_ref::<nix::Error>().map(get_nix_error_class);
      #[cfg(not(unix))]
      let maybe_get_nix_error_class = || Option::<&'static str>::None;
      (maybe_get_nix_error_class)()
    })
}
