// Copyright 2018-2026 the Deno authors. MIT license.

//! Per-API granular permission system.
//!
//! This module provides a fine-grained permission layer that sits above
//! the category-level permission system (read, write, net, env, sys, run,
//! ffi, import). Each individual API (e.g., `Deno.readFile()`,
//! `Deno.connect()`, `fetch()`) can have its own permission rule.
//!
//! ## Key types
//!
//! - [`ApiPermName`] — Stable enum of every API that checks permissions.
//!   Discriminants are stable across releases (append-only, never reorder).
//! - [`ApiPermissionResolver`] — Trait for per-API permission decisions.
//! - [`IndexedApiPermissionResolver`] — O(1) array-indexed resolver.
//! - [`CompatPermissionChecker`] — Maps every API to its category-level
//!   permission, ensuring backwards compatibility.
//!
//! ## Architecture
//!
//! ```text
//! API call (e.g., Deno.readFile("/etc/passwd"))
//!   │
//!   ▼
//! PermissionsContainer::check_open(path, Read, Some("Deno.readFile()"))
//!   │
//!   ├─► from_api_name("Deno.readFile()") → ApiPermName::DenoReadFile
//!   │
//!   ├─► ApiPermissionResolver::check(DenoReadFile, value_fn)
//!   │     │
//!   │     ├─► Allow  → skip category check, return Ok
//!   │     ├─► Deny   → return Err immediately
//!   │     └─► Defer  → fall through to category check
//!   │
//!   └─► Category-level check (existing UnaryPermission<ReadDescriptor>)
//! ```

use std::fmt;
use std::fmt::Debug;

// ---------------------------------------------------------------------------
// ApiPermName — stable enum of every permission-checked API
// ---------------------------------------------------------------------------

/// Every API that performs a permission check.
///
/// **Stability contract**: Discriminants are append-only. Once a variant is
/// assigned a number, it is never changed or removed. New APIs are always
/// added at the end with the next sequential number. This allows external
/// manifests and brokers to refer to APIs by numeric index.
///
/// Sync and async variants of the same API share a single enum value
/// (e.g., `Deno.readFile()` and `Deno.readFileSync()` both map to
/// `DenoReadFile`).
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiPermName {
  // === Read operations (check_open read, check_read_all) ===
  DenoReadFile = 0,
  DenoReadDir = 1,
  DenoReadLink = 2,
  DenoStat = 3,
  DenoLstat = 4,
  DenoRealPath = 5,
  DenoOpen = 6,
  DenoFsFileStat = 7,
  DenoWatchFs = 8,
  DenoChdir = 9,
  NodeFsExists = 10,
  NodeFsOpen = 11,
  NodeFsStatfs = 12,
  NodeSqlite = 13,
  NodeSqliteBackup = 14,

  // === Write operations (check_open write, check_write_all/partial) ===
  DenoWriteFile = 15,
  DenoMkdir = 16,
  DenoChmod = 17,
  DenoChown = 18,
  DenoRemove = 19,
  DenoRename = 20,
  DenoCopyFile = 21,
  DenoLink = 22,
  DenoTruncate = 23,
  DenoUtime = 24,
  DenoFsFileUtime = 25,
  DenoSymlink = 26,
  DenoMakeTempDir = 27,
  DenoMakeTempFile = 28,
  NodeFsLchown = 29,
  NodeFsLchmod = 30,
  NodeFsLutimes = 31,
  NodeFsMkdtemp = 32,
  NodeFsRmdir = 33,
  NodeFsCp = 34,
  NodeFsSymlink = 35,

  // === Net operations (check_net, check_net_url, check_net_vsock) ===
  DenoConnect = 36,
  DenoConnectTls = 37,
  DenoListen = 38,
  DenoListenTls = 39,
  DenoListenDatagram = 40,
  DenoDatagramSend = 41,
  DenoResolveDns = 42,
  DenoCreateHttpClient = 43,
  DenoOpenKv = 44,
  Fetch = 45,
  WebSocketNew = 46,
  WebSocketAbort = 47,
  WebSocketStreamNew = 48,
  WebSocketStreamAbort = 49,
  NodeNetListen = 50,
  NodeNetConnect = 51,
  NodeDnsLookup = 52,
  NodeDnsLookupService = 53,
  NodeDgramCreateSocket = 54,
  NodeHttpClientRequest = 55,
  InspectorOpen = 56,

  // === Sys operations (check_sys) ===
  DenoHostname = 57,
  DenoOsRelease = 58,
  DenoOsUptime = 59,
  DenoNetworkInterfaces = 60,
  DenoSystemMemoryInfo = 61,
  DenoUid = 62,
  DenoGid = 63,
  DenoLoadavg = 64,
  NodeProcessSetuid = 65,
  NodeProcessSeteuid = 66,
  NodeProcessSetgid = 67,
  NodeProcessSetegid = 68,
  NodeOsUserInfo = 69,
  NodeOsGeteuid = 70,
  NodeOsGetegid = 71,
  NodeOsGetPriority = 72,
  NodeOsSetPriority = 73,
  NodeOsCpus = 74,
  NodeOsHomedir = 75,
  InspectorUrl = 76,
  InspectorSessionConnect = 77,

  // === Run operations (check_run, check_run_all) ===
  ProcessKill = 78,

  // === Import operations (check_specifier) ===
  Import = 79,

  // === Fetch file:// (check_open read) ===
  FetchFile = 80,

  // === Process env loading (check_open read) ===
  ProcessLoadEnvFile = 81,
}

/// Total number of API variants. Update when adding new variants.
pub const API_PERM_NAME_COUNT: usize = 82;

impl ApiPermName {
  /// Convert a runtime API name string to its enum variant.
  ///
  /// Returns `None` for unrecognized strings, which causes the
  /// API resolver to be skipped (falling through to category checks).
  pub fn from_api_name(name: &str) -> Option<Self> {
    // The compiler optimizes this match into an efficient lookup.
    match name {
      // Read
      "Deno.readFile()" | "Deno.readFileSync()" => Some(Self::DenoReadFile),
      "Deno.readDir()" | "Deno.readDirSync()" => Some(Self::DenoReadDir),
      "Deno.readLink()" => Some(Self::DenoReadLink),
      "Deno.stat()" | "Deno.statSync()" => Some(Self::DenoStat),
      "Deno.lstat()" | "Deno.lstatSync()" => Some(Self::DenoLstat),
      "Deno.realPath()" | "Deno.realPathSync()" => Some(Self::DenoRealPath),
      "Deno.open()" | "Deno.openSync()" => Some(Self::DenoOpen),
      "Deno.FsFile.prototype.stat()" | "Deno.FsFile.prototype.statSync()" => {
        Some(Self::DenoFsFileStat)
      }
      "Deno.watchFs()" => Some(Self::DenoWatchFs),
      "Deno.chdir()" => Some(Self::DenoChdir),
      "node:fs.exists()" | "node:fs.existsSync()" => Some(Self::NodeFsExists),
      "node:fs.open" | "node:fs.openSync" => Some(Self::NodeFsOpen),
      "node:fs.statfs" | "node:fs.statfsSync" => Some(Self::NodeFsStatfs),
      "node:sqlite" => Some(Self::NodeSqlite),
      "node:sqlite.backup" => Some(Self::NodeSqliteBackup),

      // Write
      "Deno.writeFile()" | "Deno.writeFileSync()" => Some(Self::DenoWriteFile),
      "Deno.mkdir()" | "Deno.mkdirSync()" => Some(Self::DenoMkdir),
      "Deno.chmod()" | "Deno.chmodSync()" => Some(Self::DenoChmod),
      "Deno.chown()" | "Deno.chownSync()" => Some(Self::DenoChown),
      "Deno.remove()" | "Deno.removeSync()" => Some(Self::DenoRemove),
      "Deno.rename()" | "Deno.renameSync()" => Some(Self::DenoRename),
      "Deno.copyFile()" | "Deno.copyFileSync()" => Some(Self::DenoCopyFile),
      "Deno.link()" | "Deno.linkSync()" => Some(Self::DenoLink),
      "Deno.truncate()" | "Deno.truncateSync()" => Some(Self::DenoTruncate),
      "Deno.utime()" => Some(Self::DenoUtime),
      "Deno.FsFile.prototype.utime()" | "Deno.FsFile.prototype.utimeSync()" => {
        Some(Self::DenoFsFileUtime)
      }
      "Deno.symlink()" | "Deno.symlinkSync()" => Some(Self::DenoSymlink),
      "Deno.makeTempDir()" | "Deno.makeTempDirSync()" => {
        Some(Self::DenoMakeTempDir)
      }
      "Deno.makeTempFile()" | "Deno.makeTempFileSync()" => {
        Some(Self::DenoMakeTempFile)
      }
      "node:fs.lchown" | "node:fs.lchownSync" => Some(Self::NodeFsLchown),
      "node:fs.lchmod" | "node:fs.lchmodSync" => Some(Self::NodeFsLchmod),
      "node:fs.lutimes" | "node:fs.lutimesSync" => Some(Self::NodeFsLutimes),
      "node:fs.mkdtemp()" | "node:fs.mkdtempSync()" => {
        Some(Self::NodeFsMkdtemp)
      }
      "node:fs.rmdir" | "node:fs.rmdirSync" => Some(Self::NodeFsRmdir),
      "node:fs.cp" => Some(Self::NodeFsCp),
      "node:fs.symlink" => Some(Self::NodeFsSymlink),

      // Net
      "Deno.connect()" => Some(Self::DenoConnect),
      "Deno.connectTls()" => Some(Self::DenoConnectTls),
      "Deno.listen()" => Some(Self::DenoListen),
      "Deno.listenTls()" => Some(Self::DenoListenTls),
      "Deno.listenDatagram()" => Some(Self::DenoListenDatagram),
      "Deno.DatagramConn.send()" => Some(Self::DenoDatagramSend),
      "Deno.resolveDns()" => Some(Self::DenoResolveDns),
      "Deno.createHttpClient()" => Some(Self::DenoCreateHttpClient),
      "Deno.openKv" => Some(Self::DenoOpenKv),
      "fetch()" => Some(Self::Fetch),
      "new WebSocket()" => Some(Self::WebSocketNew),
      "WebSocket.abort()" => Some(Self::WebSocketAbort),
      "new WebSocketStream()" => Some(Self::WebSocketStreamNew),
      "WebSocketStream.abort()" => Some(Self::WebSocketStreamAbort),
      "node:net.listen()" => Some(Self::NodeNetListen),
      "node:net.connect()" => Some(Self::NodeNetConnect),
      "node:dns.lookup()" => Some(Self::NodeDnsLookup),
      "node:dns.lookupService()" => Some(Self::NodeDnsLookupService),
      "node:dgram.createSocket()" => Some(Self::NodeDgramCreateSocket),
      "ClientRequest" => Some(Self::NodeHttpClientRequest),
      "inspector.open" => Some(Self::InspectorOpen),

      // Sys
      "Deno.hostname()" => Some(Self::DenoHostname),
      "Deno.osRelease()" => Some(Self::DenoOsRelease),
      "Deno.osUptime()" => Some(Self::DenoOsUptime),
      "Deno.networkInterfaces()" => Some(Self::DenoNetworkInterfaces),
      "Deno.systemMemoryInfo()" => Some(Self::DenoSystemMemoryInfo),
      "Deno.uid()" => Some(Self::DenoUid),
      "Deno.gid()" => Some(Self::DenoGid),
      "Deno.loadavg()" => Some(Self::DenoLoadavg),
      "node:process.setuid" => Some(Self::NodeProcessSetuid),
      "node:process.seteuid" => Some(Self::NodeProcessSeteuid),
      "node:process.setgid" => Some(Self::NodeProcessSetgid),
      "node:process.setegid" => Some(Self::NodeProcessSetegid),
      "node:os.userInfo()" => Some(Self::NodeOsUserInfo),
      "node:os.geteuid()" => Some(Self::NodeOsGeteuid),
      "node:os.getegid()" => Some(Self::NodeOsGetegid),
      "node:os.getPriority()" => Some(Self::NodeOsGetPriority),
      "node:os.setPriority()" => Some(Self::NodeOsSetPriority),
      "node:os.cpus()" => Some(Self::NodeOsCpus),
      "node:os.homedir()" => Some(Self::NodeOsHomedir),
      "inspector.url" => Some(Self::InspectorUrl),
      "inspector.Session.connect" => Some(Self::InspectorSessionConnect),

      // Run
      "process.kill" => Some(Self::ProcessKill),

      // Import
      "import()" => Some(Self::Import),

      // Fetch file://
      "fetch() file" => Some(Self::FetchFile),

      // Env file loading
      "process.loadEnvFile" => Some(Self::ProcessLoadEnvFile),

      _ => None,
    }
  }

  /// Returns the index of this API (same as the discriminant).
  #[inline(always)]
  pub const fn index(self) -> usize {
    self as usize
  }

  /// Returns the canonical API name string.
  pub const fn api_name(self) -> &'static str {
    match self {
      Self::DenoReadFile => "Deno.readFile()",
      Self::DenoReadDir => "Deno.readDir()",
      Self::DenoReadLink => "Deno.readLink()",
      Self::DenoStat => "Deno.stat()",
      Self::DenoLstat => "Deno.lstat()",
      Self::DenoRealPath => "Deno.realPath()",
      Self::DenoOpen => "Deno.open()",
      Self::DenoFsFileStat => "Deno.FsFile.prototype.stat()",
      Self::DenoWatchFs => "Deno.watchFs()",
      Self::DenoChdir => "Deno.chdir()",
      Self::NodeFsExists => "node:fs.exists()",
      Self::NodeFsOpen => "node:fs.open",
      Self::NodeFsStatfs => "node:fs.statfs",
      Self::NodeSqlite => "node:sqlite",
      Self::NodeSqliteBackup => "node:sqlite.backup",
      Self::DenoWriteFile => "Deno.writeFile()",
      Self::DenoMkdir => "Deno.mkdir()",
      Self::DenoChmod => "Deno.chmod()",
      Self::DenoChown => "Deno.chown()",
      Self::DenoRemove => "Deno.remove()",
      Self::DenoRename => "Deno.rename()",
      Self::DenoCopyFile => "Deno.copyFile()",
      Self::DenoLink => "Deno.link()",
      Self::DenoTruncate => "Deno.truncate()",
      Self::DenoUtime => "Deno.utime()",
      Self::DenoFsFileUtime => "Deno.FsFile.prototype.utime()",
      Self::DenoSymlink => "Deno.symlink()",
      Self::DenoMakeTempDir => "Deno.makeTempDir()",
      Self::DenoMakeTempFile => "Deno.makeTempFile()",
      Self::NodeFsLchown => "node:fs.lchown",
      Self::NodeFsLchmod => "node:fs.lchmod",
      Self::NodeFsLutimes => "node:fs.lutimes",
      Self::NodeFsMkdtemp => "node:fs.mkdtemp()",
      Self::NodeFsRmdir => "node:fs.rmdir",
      Self::NodeFsCp => "node:fs.cp",
      Self::NodeFsSymlink => "node:fs.symlink",
      Self::DenoConnect => "Deno.connect()",
      Self::DenoConnectTls => "Deno.connectTls()",
      Self::DenoListen => "Deno.listen()",
      Self::DenoListenTls => "Deno.listenTls()",
      Self::DenoListenDatagram => "Deno.listenDatagram()",
      Self::DenoDatagramSend => "Deno.DatagramConn.send()",
      Self::DenoResolveDns => "Deno.resolveDns()",
      Self::DenoCreateHttpClient => "Deno.createHttpClient()",
      Self::DenoOpenKv => "Deno.openKv",
      Self::Fetch => "fetch()",
      Self::WebSocketNew => "new WebSocket()",
      Self::WebSocketAbort => "WebSocket.abort()",
      Self::WebSocketStreamNew => "new WebSocketStream()",
      Self::WebSocketStreamAbort => "WebSocketStream.abort()",
      Self::NodeNetListen => "node:net.listen()",
      Self::NodeNetConnect => "node:net.connect()",
      Self::NodeDnsLookup => "node:dns.lookup()",
      Self::NodeDnsLookupService => "node:dns.lookupService()",
      Self::NodeDgramCreateSocket => "node:dgram.createSocket()",
      Self::NodeHttpClientRequest => "ClientRequest",
      Self::InspectorOpen => "inspector.open",
      Self::DenoHostname => "Deno.hostname()",
      Self::DenoOsRelease => "Deno.osRelease()",
      Self::DenoOsUptime => "Deno.osUptime()",
      Self::DenoNetworkInterfaces => "Deno.networkInterfaces()",
      Self::DenoSystemMemoryInfo => "Deno.systemMemoryInfo()",
      Self::DenoUid => "Deno.uid()",
      Self::DenoGid => "Deno.gid()",
      Self::DenoLoadavg => "Deno.loadavg()",
      Self::NodeProcessSetuid => "node:process.setuid",
      Self::NodeProcessSeteuid => "node:process.seteuid",
      Self::NodeProcessSetgid => "node:process.setgid",
      Self::NodeProcessSetegid => "node:process.setegid",
      Self::NodeOsUserInfo => "node:os.userInfo()",
      Self::NodeOsGeteuid => "node:os.geteuid()",
      Self::NodeOsGetegid => "node:os.getegid()",
      Self::NodeOsGetPriority => "node:os.getPriority()",
      Self::NodeOsSetPriority => "node:os.setPriority()",
      Self::NodeOsCpus => "node:os.cpus()",
      Self::NodeOsHomedir => "node:os.homedir()",
      Self::InspectorUrl => "inspector.url",
      Self::InspectorSessionConnect => "inspector.Session.connect",
      Self::ProcessKill => "process.kill",
      Self::Import => "import()",
      Self::FetchFile => "fetch()",
      Self::ProcessLoadEnvFile => "process.loadEnvFile",
    }
  }
}

impl fmt::Display for ApiPermName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.api_name())
  }
}

// ---------------------------------------------------------------------------
// CompatPermissionChecker — exhaustive API → category mapping
// ---------------------------------------------------------------------------

/// Maps every [`ApiPermName`] to its permission category(ies).
///
/// This is the single source of truth for backwards compatibility:
/// it defines which category-level permission (`--allow-read`, etc.)
/// governs each API. Adding a new [`ApiPermName`] variant without
/// updating this match will cause a compile error.
pub struct CompatPermissionChecker;

impl CompatPermissionChecker {
  /// Returns the primary permission category for an API.
  ///
  /// This is used for error messages and audit logging when the
  /// per-API resolver makes a decision.
  pub const fn primary_category(api: ApiPermName) -> &'static str {
    match api {
      // Read
      ApiPermName::DenoReadFile => "read",
      ApiPermName::DenoReadDir => "read",
      ApiPermName::DenoReadLink => "read",
      ApiPermName::DenoStat => "read",
      ApiPermName::DenoLstat => "read",
      ApiPermName::DenoRealPath => "read",
      ApiPermName::DenoOpen => "read",
      ApiPermName::DenoFsFileStat => "read",
      ApiPermName::DenoWatchFs => "read",
      ApiPermName::DenoChdir => "read",
      ApiPermName::NodeFsExists => "read",
      ApiPermName::NodeFsOpen => "read",
      ApiPermName::NodeFsStatfs => "read",
      ApiPermName::NodeSqlite => "ffi",
      ApiPermName::NodeSqliteBackup => "write",
      ApiPermName::FetchFile => "read",
      ApiPermName::ProcessLoadEnvFile => "read",
      ApiPermName::Import => "import",

      // Write
      ApiPermName::DenoWriteFile => "write",
      ApiPermName::DenoMkdir => "write",
      ApiPermName::DenoChmod => "write",
      ApiPermName::DenoChown => "write",
      ApiPermName::DenoRemove => "write",
      ApiPermName::DenoRename => "write",
      ApiPermName::DenoCopyFile => "write",
      ApiPermName::DenoLink => "write",
      ApiPermName::DenoTruncate => "write",
      ApiPermName::DenoUtime => "write",
      ApiPermName::DenoFsFileUtime => "write",
      ApiPermName::DenoSymlink => "write",
      ApiPermName::DenoMakeTempDir => "write",
      ApiPermName::DenoMakeTempFile => "write",
      ApiPermName::NodeFsLchown => "write",
      ApiPermName::NodeFsLchmod => "write",
      ApiPermName::NodeFsLutimes => "write",
      ApiPermName::NodeFsMkdtemp => "write",
      ApiPermName::NodeFsRmdir => "write",
      ApiPermName::NodeFsCp => "write",
      ApiPermName::NodeFsSymlink => "write",

      // Net
      ApiPermName::DenoConnect => "net",
      ApiPermName::DenoConnectTls => "net",
      ApiPermName::DenoListen => "net",
      ApiPermName::DenoListenTls => "net",
      ApiPermName::DenoListenDatagram => "net",
      ApiPermName::DenoDatagramSend => "net",
      ApiPermName::DenoResolveDns => "net",
      ApiPermName::DenoCreateHttpClient => "net",
      ApiPermName::DenoOpenKv => "net",
      ApiPermName::Fetch => "net",
      ApiPermName::WebSocketNew => "net",
      ApiPermName::WebSocketAbort => "net",
      ApiPermName::WebSocketStreamNew => "net",
      ApiPermName::WebSocketStreamAbort => "net",
      ApiPermName::NodeNetListen => "net",
      ApiPermName::NodeNetConnect => "net",
      ApiPermName::NodeDnsLookup => "net",
      ApiPermName::NodeDnsLookupService => "net",
      ApiPermName::NodeDgramCreateSocket => "net",
      ApiPermName::NodeHttpClientRequest => "net",
      ApiPermName::InspectorOpen => "net",

      // Sys
      ApiPermName::DenoHostname => "sys",
      ApiPermName::DenoOsRelease => "sys",
      ApiPermName::DenoOsUptime => "sys",
      ApiPermName::DenoNetworkInterfaces => "sys",
      ApiPermName::DenoSystemMemoryInfo => "sys",
      ApiPermName::DenoUid => "sys",
      ApiPermName::DenoGid => "sys",
      ApiPermName::DenoLoadavg => "sys",
      ApiPermName::NodeProcessSetuid => "sys",
      ApiPermName::NodeProcessSeteuid => "sys",
      ApiPermName::NodeProcessSetgid => "sys",
      ApiPermName::NodeProcessSetegid => "sys",
      ApiPermName::NodeOsUserInfo => "sys",
      ApiPermName::NodeOsGeteuid => "sys",
      ApiPermName::NodeOsGetegid => "sys",
      ApiPermName::NodeOsGetPriority => "sys",
      ApiPermName::NodeOsSetPriority => "sys",
      ApiPermName::NodeOsCpus => "sys",
      ApiPermName::NodeOsHomedir => "sys",
      ApiPermName::InspectorUrl => "sys",
      ApiPermName::InspectorSessionConnect => "sys",

      // Run
      ApiPermName::ProcessKill => "run",
    }
  }

  /// Returns all permission categories this API checks against.
  ///
  /// Most APIs check a single category. Some check multiple
  /// (e.g., `Deno.symlink()` checks both read and write).
  pub fn categories(api: ApiPermName) -> &'static [&'static str] {
    match api {
      // Multi-category APIs
      ApiPermName::DenoSymlink => &["read", "write"],
      ApiPermName::DenoOpen => &["read", "write"],
      ApiPermName::DenoCopyFile => &["read", "write"],
      ApiPermName::DenoRename => &["read", "write"],
      ApiPermName::DenoLink => &["read", "write"],
      ApiPermName::NodeFsCp => &["read", "write"],
      ApiPermName::NodeFsSymlink => &["read", "write"],
      ApiPermName::NodeFsStatfs => &["read", "sys"],
      ApiPermName::Fetch => &["net", "read"],

      // Single-category APIs
      ApiPermName::DenoReadFile
      | ApiPermName::DenoReadDir
      | ApiPermName::DenoReadLink
      | ApiPermName::DenoStat
      | ApiPermName::DenoLstat
      | ApiPermName::DenoRealPath
      | ApiPermName::DenoFsFileStat
      | ApiPermName::DenoWatchFs
      | ApiPermName::DenoChdir
      | ApiPermName::NodeFsExists
      | ApiPermName::NodeFsOpen
      | ApiPermName::FetchFile
      | ApiPermName::ProcessLoadEnvFile => &["read"],

      ApiPermName::DenoWriteFile
      | ApiPermName::DenoMkdir
      | ApiPermName::DenoChmod
      | ApiPermName::DenoChown
      | ApiPermName::DenoRemove
      | ApiPermName::DenoTruncate
      | ApiPermName::DenoUtime
      | ApiPermName::DenoFsFileUtime
      | ApiPermName::DenoMakeTempDir
      | ApiPermName::DenoMakeTempFile
      | ApiPermName::NodeFsLchown
      | ApiPermName::NodeFsLchmod
      | ApiPermName::NodeFsLutimes
      | ApiPermName::NodeFsMkdtemp
      | ApiPermName::NodeFsRmdir
      | ApiPermName::NodeSqliteBackup => &["write"],

      ApiPermName::DenoConnect
      | ApiPermName::DenoConnectTls
      | ApiPermName::DenoListen
      | ApiPermName::DenoListenTls
      | ApiPermName::DenoListenDatagram
      | ApiPermName::DenoDatagramSend
      | ApiPermName::DenoResolveDns
      | ApiPermName::DenoCreateHttpClient
      | ApiPermName::DenoOpenKv
      | ApiPermName::WebSocketNew
      | ApiPermName::WebSocketAbort
      | ApiPermName::WebSocketStreamNew
      | ApiPermName::WebSocketStreamAbort
      | ApiPermName::NodeNetListen
      | ApiPermName::NodeNetConnect
      | ApiPermName::NodeDnsLookup
      | ApiPermName::NodeDnsLookupService
      | ApiPermName::NodeDgramCreateSocket
      | ApiPermName::NodeHttpClientRequest
      | ApiPermName::InspectorOpen => &["net"],

      ApiPermName::DenoHostname
      | ApiPermName::DenoOsRelease
      | ApiPermName::DenoOsUptime
      | ApiPermName::DenoNetworkInterfaces
      | ApiPermName::DenoSystemMemoryInfo
      | ApiPermName::DenoUid
      | ApiPermName::DenoGid
      | ApiPermName::DenoLoadavg
      | ApiPermName::NodeProcessSetuid
      | ApiPermName::NodeProcessSeteuid
      | ApiPermName::NodeProcessSetgid
      | ApiPermName::NodeProcessSetegid
      | ApiPermName::NodeOsUserInfo
      | ApiPermName::NodeOsGeteuid
      | ApiPermName::NodeOsGetegid
      | ApiPermName::NodeOsGetPriority
      | ApiPermName::NodeOsSetPriority
      | ApiPermName::NodeOsCpus
      | ApiPermName::NodeOsHomedir
      | ApiPermName::InspectorUrl
      | ApiPermName::InspectorSessionConnect => &["sys"],

      ApiPermName::ProcessKill => &["run"],
      ApiPermName::NodeSqlite => &["ffi"],
      ApiPermName::Import => &["import"],
    }
  }
}

// ---------------------------------------------------------------------------
// ApiCheckResult / ApiRule
// ---------------------------------------------------------------------------

/// Result of an API-level permission check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiCheckResult {
  /// This specific API call is allowed; skip category-level checks.
  Allow,
  /// This specific API call is denied.
  Deny { reason: Option<String> },
  /// No per-API rule exists; defer to the category-level permission system.
  Defer,
}

/// A rule for a specific API, as stored in a manifest.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ApiRule {
  Allow,
  Deny {
    #[serde(default)]
    reason: Option<String>,
  },
}

// ---------------------------------------------------------------------------
// ApiPermissionResolver trait
// ---------------------------------------------------------------------------

/// Trait for resolving per-API permissions.
///
/// Implementations can be backed by a JSON manifest, an external broker
/// process, or any other mechanism.
pub trait ApiPermissionResolver: Send + Sync + Debug {
  /// Check whether a specific API call should be allowed, denied, or
  /// deferred to the category-level permission system.
  ///
  /// # Arguments
  /// * `api` - The API being called, identified by its stable enum variant.
  /// * `value_fn` - Lazy function returning the stringified resource
  ///   descriptor (e.g., a file path or hostname). Only called if
  ///   the resolver needs the value.
  fn check(
    &self,
    api: ApiPermName,
    value_fn: &dyn Fn() -> Option<String>,
  ) -> ApiCheckResult;
}

// ---------------------------------------------------------------------------
// IndexedApiPermissionResolver — O(1) array-indexed resolver
// ---------------------------------------------------------------------------

/// A resolver backed by a fixed-size array indexed by [`ApiPermName`].
///
/// Lookups are a single array index operation — no hashing, no string
/// comparison. This is the fastest possible resolver implementation.
///
/// # Example
///
/// ```ignore
/// use deno_permissions::api::*;
///
/// let mut resolver = IndexedApiPermissionResolver::new();
/// resolver.set(ApiPermName::DenoReadDir, ApiRule::Deny {
///   reason: Some("directory listing not allowed".into()),
/// });
/// resolver.set(ApiPermName::Fetch, ApiRule::Allow);
/// ```
#[derive(Debug, Clone)]
pub struct IndexedApiPermissionResolver {
  rules: Box<[Option<ApiRule>; API_PERM_NAME_COUNT]>,
}

impl IndexedApiPermissionResolver {
  /// Create a new resolver with no rules (all APIs defer).
  pub fn new() -> Self {
    Self {
      rules: Box::new(std::array::from_fn(|_| None)),
    }
  }

  /// Set a rule for a specific API.
  pub fn set(&mut self, api: ApiPermName, rule: ApiRule) {
    self.rules[api.index()] = Some(rule);
  }

  /// Remove the rule for a specific API (reverts to defer).
  pub fn clear(&mut self, api: ApiPermName) {
    self.rules[api.index()] = None;
  }

  /// Build from a JSON manifest. The manifest maps API name strings
  /// to rules. Unrecognized API names are silently ignored.
  ///
  /// ```json
  /// {
  ///   "Deno.readFile()": { "state": "allow" },
  ///   "Deno.readDir()": { "state": "deny", "reason": "not allowed" }
  /// }
  /// ```
  pub fn from_json(json: &str) -> Result<Self, IndexedResolverFromJsonError> {
    let map: std::collections::HashMap<String, ApiRule> =
      serde_json::from_str(json)?;
    let mut resolver = Self::new();
    for (name, rule) in map {
      if let Some(api) = ApiPermName::from_api_name(&name) {
        resolver.set(api, rule);
      }
    }
    Ok(resolver)
  }
}

impl Default for IndexedApiPermissionResolver {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct IndexedResolverFromJsonError(#[from] serde_json::Error);

impl ApiPermissionResolver for IndexedApiPermissionResolver {
  #[inline(always)]
  fn check(
    &self,
    api: ApiPermName,
    _value_fn: &dyn Fn() -> Option<String>,
  ) -> ApiCheckResult {
    match &self.rules[api.index()] {
      Some(ApiRule::Allow) => ApiCheckResult::Allow,
      Some(ApiRule::Deny { reason }) => ApiCheckResult::Deny {
        reason: reason.clone(),
      },
      None => ApiCheckResult::Defer,
    }
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_api_perm_name_count_matches_variants() {
    // Ensure API_PERM_NAME_COUNT is correct by checking the last variant.
    assert_eq!(
      ApiPermName::ProcessLoadEnvFile.index(),
      API_PERM_NAME_COUNT - 1,
      "API_PERM_NAME_COUNT must equal the last variant's index + 1"
    );
  }

  #[test]
  fn test_from_api_name_roundtrip() {
    // Verify that canonical names roundtrip through from_api_name
    let apis = [
      ApiPermName::DenoReadFile,
      ApiPermName::DenoConnect,
      ApiPermName::Fetch,
      ApiPermName::DenoHostname,
      ApiPermName::ProcessKill,
      ApiPermName::Import,
    ];
    for api in apis {
      let name = api.api_name();
      let resolved = ApiPermName::from_api_name(name);
      assert_eq!(resolved, Some(api), "roundtrip failed for {name}");
    }
  }

  #[test]
  fn test_from_api_name_sync_variants() {
    // Sync and async variants map to the same enum value
    assert_eq!(
      ApiPermName::from_api_name("Deno.readFile()"),
      ApiPermName::from_api_name("Deno.readFileSync()"),
    );
    assert_eq!(
      ApiPermName::from_api_name("Deno.stat()"),
      ApiPermName::from_api_name("Deno.statSync()"),
    );
    assert_eq!(
      ApiPermName::from_api_name("node:fs.open"),
      ApiPermName::from_api_name("node:fs.openSync"),
    );
  }

  #[test]
  fn test_from_api_name_unknown_returns_none() {
    assert_eq!(ApiPermName::from_api_name("unknown.api()"), None);
  }

  #[test]
  fn test_indexed_resolver_basic() {
    let mut resolver = IndexedApiPermissionResolver::new();
    resolver.set(ApiPermName::DenoReadFile, ApiRule::Allow);
    resolver.set(
      ApiPermName::DenoReadDir,
      ApiRule::Deny {
        reason: Some("not allowed".to_string()),
      },
    );

    assert_eq!(
      resolver.check(ApiPermName::DenoReadFile, &|| None),
      ApiCheckResult::Allow,
    );
    assert_eq!(
      resolver.check(ApiPermName::DenoReadDir, &|| None),
      ApiCheckResult::Deny {
        reason: Some("not allowed".to_string()),
      },
    );
    assert_eq!(
      resolver.check(ApiPermName::DenoWriteFile, &|| None),
      ApiCheckResult::Defer,
    );
  }

  #[test]
  fn test_indexed_resolver_from_json() {
    let json = r#"{
      "Deno.readFile()": { "state": "allow" },
      "fetch()": { "state": "deny", "reason": "no fetch allowed" },
      "unknown.api()": { "state": "allow" }
    }"#;

    let resolver = IndexedApiPermissionResolver::from_json(json).unwrap();

    assert_eq!(
      resolver.check(ApiPermName::DenoReadFile, &|| None),
      ApiCheckResult::Allow,
    );
    assert_eq!(
      resolver.check(ApiPermName::Fetch, &|| None),
      ApiCheckResult::Deny {
        reason: Some("no fetch allowed".to_string()),
      },
    );
    // Unknown API in manifest is silently ignored
    assert_eq!(
      resolver.check(ApiPermName::DenoConnect, &|| None),
      ApiCheckResult::Defer,
    );
  }

  #[test]
  fn test_indexed_resolver_value_fn_not_called() {
    let mut resolver = IndexedApiPermissionResolver::new();
    resolver.set(ApiPermName::DenoReadFile, ApiRule::Allow);

    let result =
      resolver.check(ApiPermName::DenoReadFile, &|| panic!("should not call"));
    assert_eq!(result, ApiCheckResult::Allow);
  }

  #[test]
  fn test_indexed_resolver_clear() {
    let mut resolver = IndexedApiPermissionResolver::new();
    resolver.set(ApiPermName::DenoReadFile, ApiRule::Allow);
    assert_eq!(
      resolver.check(ApiPermName::DenoReadFile, &|| None),
      ApiCheckResult::Allow,
    );

    resolver.clear(ApiPermName::DenoReadFile);
    assert_eq!(
      resolver.check(ApiPermName::DenoReadFile, &|| None),
      ApiCheckResult::Defer,
    );
  }

  #[test]
  fn test_compat_checker_primary_category() {
    assert_eq!(
      CompatPermissionChecker::primary_category(ApiPermName::DenoReadFile),
      "read"
    );
    assert_eq!(
      CompatPermissionChecker::primary_category(ApiPermName::DenoWriteFile),
      "write"
    );
    assert_eq!(
      CompatPermissionChecker::primary_category(ApiPermName::DenoConnect),
      "net"
    );
    assert_eq!(
      CompatPermissionChecker::primary_category(ApiPermName::DenoHostname),
      "sys"
    );
    assert_eq!(
      CompatPermissionChecker::primary_category(ApiPermName::ProcessKill),
      "run"
    );
    assert_eq!(
      CompatPermissionChecker::primary_category(ApiPermName::NodeSqlite),
      "ffi"
    );
    assert_eq!(
      CompatPermissionChecker::primary_category(ApiPermName::Import),
      "import"
    );
  }

  #[test]
  fn test_compat_checker_multi_category() {
    let cats = CompatPermissionChecker::categories(ApiPermName::DenoSymlink);
    assert_eq!(cats, &["read", "write"]);

    let cats = CompatPermissionChecker::categories(ApiPermName::Fetch);
    assert_eq!(cats, &["net", "read"]);

    let cats = CompatPermissionChecker::categories(ApiPermName::DenoReadFile);
    assert_eq!(cats, &["read"]);
  }

  #[test]
  fn test_all_apis_have_consistent_category() {
    // Verify that primary_category returns one of the expected values
    // for every single API variant. This exercises exhaustiveness.
    let valid_categories =
      ["read", "write", "net", "sys", "run", "ffi", "import"];
    for i in 0..API_PERM_NAME_COUNT {
      // Safety: we iterate within the valid range
      let api = unsafe { std::mem::transmute::<u16, ApiPermName>(i as u16) };
      let cat = CompatPermissionChecker::primary_category(api);
      assert!(
        valid_categories.contains(&cat),
        "API {:?} has unexpected category: {}",
        api,
        cat
      );
      // categories() should include the primary
      let cats = CompatPermissionChecker::categories(api);
      assert!(
        cats.contains(&cat),
        "categories() for {:?} doesn't include primary_category {}",
        api,
        cat
      );
    }
  }
}
