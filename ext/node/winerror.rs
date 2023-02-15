use deno_core::op;

#[op]
fn op_node_sys_to_uv_error(err: i32) -> String {
  let uv_err = match err {
    ERROR_ACCESS_DENIED => "EACCES",
    ERROR_NOACCESS => "EACCES",
    WSAEACCES => "EACCES",
    ERROR_CANT_ACCESS_FILE => "EACCES",
    ERROR_ADDRESS_ALREADY_ASSOCIATED => "EADDRINUSE",
    WSAEADDRINUSE => "EADDRINUSE",
    WSAEADDRNOTAVAIL => "EADDRNOTAVAIL",
    WSAEAFNOSUPPORT => "EAFNOSUPPORT",
    WSAEWOULDBLOCK => "EAGAIN",
    WSAEALREADY => "EALREADY",
    ERROR_INVALID_FLAGS => "EBADF",
    ERROR_INVALID_HANDLE => "EBADF",
    ERROR_LOCK_VIOLATION => "EBUSY",
    ERROR_PIPE_BUSY => "EBUSY",
    ERROR_SHARING_VIOLATION => "EBUSY",
    ERROR_OPERATION_ABORTED => "ECANCELED",
    WSAEINTR => "ECANCELED",
    ERROR_NO_UNICODE_TRANSLATION => "ECHARSET",
    ERROR_CONNECTION_ABORTED => "ECONNABORTED",
    WSAECONNABORTED => "ECONNABORTED",
    ERROR_CONNECTION_REFUSED => "ECONNREFUSED",
    WSAECONNREFUSED => "ECONNREFUSED",
    ERROR_NETNAME_DELETED => "ECONNRESET",
    WSAECONNRESET => "ECONNRESET",
    ERROR_ALREADY_EXISTS => "EEXIST",
    ERROR_FILE_EXISTS => "EEXIST",
    ERROR_BUFFER_OVERFLOW => "EFAULT",
    WSAEFAULT => "EFAULT",
    ERROR_HOST_UNREACHABLE => "EHOSTUNREACH",
    WSAEHOSTUNREACH => "EHOSTUNREACH",
    ERROR_INSUFFICIENT_BUFFER => "EINVAL",
    ERROR_INVALID_DATA => "EINVAL",
    ERROR_INVALID_NAME => "EINVAL",
    ERROR_INVALID_PARAMETER => "EINVAL",
    WSAEINVAL => "EINVAL",
    WSAEPFNOSUPPORT => "EINVAL",
    ERROR_BEGINNING_OF_MEDIA => "EIO",
    ERROR_BUS_RESET => "EIO",
    ERROR_CRC => "EIO",
    ERROR_DEVICE_DOOR_OPEN => "EIO",
    ERROR_DEVICE_REQUIRES_CLEANING => "EIO",
    ERROR_DISK_CORRUPT => "EIO",
    ERROR_EOM_OVERFLOW => "EIO",
    ERROR_FILEMARK_DETECTED => "EIO",
    ERROR_GEN_FAILURE => "EIO",
    ERROR_INVALID_BLOCK_LENGTH => "EIO",
    ERROR_IO_DEVICE => "EIO",
    ERROR_NO_DATA_DETECTED => "EIO",
    ERROR_NO_SIGNAL_SENT => "EIO",
    ERROR_OPEN_FAILED => "EIO",
    ERROR_SETMARK_DETECTED => "EIO",
    ERROR_SIGNAL_REFUSED => "EIO",
    WSAEISCONN => "EISCONN",
    ERROR_CANT_RESOLVE_FILENAME => "ELOOP",
    ERROR_TOO_MANY_OPEN_FILES => "EMFILE",
    WSAEMFILE => "EMFILE",
    WSAEMSGSIZE => "EMSGSIZE",
    ERROR_FILENAME_EXCED_RANGE => "ENAMETOOLONG",
    ERROR_NETWORK_UNREACHABLE => "ENETUNREACH",
    WSAENETUNREACH => "ENETUNREACH",
    WSAENOBUFS => "ENOBUFS",
    ERROR_BAD_PATHNAME => "ENOENT",
    ERROR_DIRECTORY => "ENOTDIR",
    ERROR_ENVVAR_NOT_FOUND => "ENOENT",
    ERROR_FILE_NOT_FOUND => "ENOENT",
    ERROR_INVALID_DRIVE => "ENOENT",
    ERROR_INVALID_REPARSE_DATA => "ENOENT",
    ERROR_MOD_NOT_FOUND => "ENOENT",
    ERROR_PATH_NOT_FOUND => "ENOENT",
    WSAHOST_NOT_FOUND => "ENOENT",
    WSANO_DATA => "ENOENT",
    ERROR_NOT_ENOUGH_MEMORY => "ENOMEM",
    ERROR_OUTOFMEMORY => "ENOMEM",
    ERROR_CANNOT_MAKE => "ENOSPC",
    ERROR_DISK_FULL => "ENOSPC",
    ERROR_EA_TABLE_FULL => "ENOSPC",
    ERROR_END_OF_MEDIA => "ENOSPC",
    ERROR_HANDLE_DISK_FULL => "ENOSPC",
    ERROR_NOT_CONNECTED => "ENOTCONN",
    WSAENOTCONN => "ENOTCONN",
    ERROR_DIR_NOT_EMPTY => "ENOTEMPTY",
    WSAENOTSOCK => "ENOTSOCK",
    ERROR_NOT_SUPPORTED => "ENOTSUP",
    ERROR_BROKEN_PIPE => "EOF",
    ERROR_PRIVILEGE_NOT_HELD => "EPERM",
    ERROR_BAD_PIPE => "EPIPE",
    ERROR_NO_DATA => "EPIPE",
    ERROR_PIPE_NOT_CONNECTED => "EPIPE",
    WSAESHUTDOWN => "EPIPE",
    WSAEPROTONOSUPPORT => "EPROTONOSUPPORT",
    ERROR_WRITE_PROTECT => "EROFS",
    ERROR_SEM_TIMEOUT => "ETIMEDOUT",
    WSAETIMEDOUT => "ETIMEDOUT",
    ERROR_NOT_SAME_DEVICE => "EXDEV",
    ERROR_INVALID_FUNCTION => "EISDIR",
    ERROR_META_EXPANSION_TOO_LONG => "E2BIG",
    WSAESOCKTNOSUPPORT => "ESOCKTNOSUPPORT",
    _ => "UNKNOWN",
  };
  uv_err.to_string()
}

pub const ERROR_IPSEC_IKE_SRVQUERYCRED: i32 = 13856;

///
/// MessageId: ERROR_IPSEC_IKE_GETSPIFAIL
///
/// MessageText:
///
///  Failed to obtain new SPI for the inbound SA from Ipsec driver.  The most common cause for this is that the driver does not have the correct filter.  Check your policy to verify the filters.
///
pub const ERROR_IPSEC_IKE_GETSPIFAIL: i32 = 13857;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_FILTER
///
/// MessageText:
///
///  Given filter is invalid
///
pub const ERROR_IPSEC_IKE_INVALID_FILTER: i32 = 13858;

///
/// MessageId: ERROR_IPSEC_IKE_OUT_OF_MEMORY
///
/// MessageText:
///
///  Memory allocation failed.
///
pub const ERROR_IPSEC_IKE_OUT_OF_MEMORY: i32 = 13859;

///
/// MessageId: ERROR_IPSEC_IKE_ADD_UPDATE_KEY_FAILED
///
/// MessageText:
///
///  Failed to add Security Association to IPSec Driver.  The most common cause for this is if the IKE negotiation took too long to complete.  If the problem persists, reduce the load on the faulting machine.
///
pub const ERROR_IPSEC_IKE_ADD_UPDATE_KEY_FAILED: i32 = 13860;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_POLICY
///
/// MessageText:
///
///  Invalid policy
///
pub const ERROR_IPSEC_IKE_INVALID_POLICY: i32 = 13861;

///
/// MessageId: ERROR_IPSEC_IKE_UNKNOWN_DOI
///
/// MessageText:
///
///  Invalid DOI
///
pub const ERROR_IPSEC_IKE_UNKNOWN_DOI: i32 = 13862;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_SITUATION
///
/// MessageText:
///
///  Invalid situation
///
pub const ERROR_IPSEC_IKE_INVALID_SITUATION: i32 = 13863;

///
/// MessageId: ERROR_IPSEC_IKE_DH_FAILURE
///
/// MessageText:
///
///  Diffie-Hellman failure
///
pub const ERROR_IPSEC_IKE_DH_FAILURE: i32 = 13864;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_GROUP
///
/// MessageText:
///
///  Invalid Diffie-Hellman group
///
pub const ERROR_IPSEC_IKE_INVALID_GROUP: i32 = 13865;

///
/// MessageId: ERROR_IPSEC_IKE_ENCRYPT
///
/// MessageText:
///
///  Error encrypting payload
///
pub const ERROR_IPSEC_IKE_ENCRYPT: i32 = 13866;

///
/// MessageId: ERROR_IPSEC_IKE_DECRYPT
///
/// MessageText:
///
///  Error decrypting payload
///
pub const ERROR_IPSEC_IKE_DECRYPT: i32 = 13867;

///
/// MessageId: ERROR_IPSEC_IKE_POLICY_MATCH
///
/// MessageText:
///
///  Policy match error
///
pub const ERROR_IPSEC_IKE_POLICY_MATCH: i32 = 13868;

///
/// MessageId: ERROR_IPSEC_IKE_UNSUPPORTED_ID
///
/// MessageText:
///
///  Unsupported ID
///
pub const ERROR_IPSEC_IKE_UNSUPPORTED_ID: i32 = 13869;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_HASH
///
/// MessageText:
///
///  Hash verification failed
///
pub const ERROR_IPSEC_IKE_INVALID_HASH: i32 = 13870;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_HASH_ALG
///
/// MessageText:
///
///  Invalid hash algorithm
///
pub const ERROR_IPSEC_IKE_INVALID_HASH_ALG: i32 = 13871;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_HASH_SIZE
///
/// MessageText:
///
///  Invalid hash size
///
pub const ERROR_IPSEC_IKE_INVALID_HASH_SIZE: i32 = 13872;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_ENCRYPT_ALG
///
/// MessageText:
///
///  Invalid encryption algorithm
///
pub const ERROR_IPSEC_IKE_INVALID_ENCRYPT_ALG: i32 = 13873;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_AUTH_ALG
///
/// MessageText:
///
///  Invalid authentication algorithm
///
pub const ERROR_IPSEC_IKE_INVALID_AUTH_ALG: i32 = 13874;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_SIG
///
/// MessageText:
///
///  Invalid certificate signature
///
pub const ERROR_IPSEC_IKE_INVALID_SIG: i32 = 13875;

///
/// MessageId: ERROR_IPSEC_IKE_LOAD_FAILED
///
/// MessageText:
///
///  Load failed
///
pub const ERROR_IPSEC_IKE_LOAD_FAILED: i32 = 13876;

///
/// MessageId: ERROR_IPSEC_IKE_RPC_DELETE
///
/// MessageText:
///
///  Deleted via RPC call
///
pub const ERROR_IPSEC_IKE_RPC_DELETE: i32 = 13877;

///
/// MessageId: ERROR_IPSEC_IKE_BENIGN_REINIT
///
/// MessageText:
///
///  Temporary state created to perform reinit. This is not a real failure.
///
pub const ERROR_IPSEC_IKE_BENIGN_REINIT: i32 = 13878;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_RESPONDER_LIFETIME_NOTIFY
///
/// MessageText:
///
///  The lifetime value received in the Responder Lifetime Notify is below the Windows 2000 configured minimum value.  Please fix the policy on the peer machine.
///
pub const ERROR_IPSEC_IKE_INVALID_RESPONDER_LIFETIME_NOTIFY: i32 = 13879;

///
/// MessageId: ERROR_IPSEC_IKE_INVALID_CERT_KEYLEN
///
/// MessageText:
///
///  Key length in certificate is too small for configured security requirements.
///
pub const ERROR_IPSEC_IKE_INVALID_CERT_KEYLEN: i32 = 13881;

///
/// MessageId: ERROR_IPSEC_IKE_MM_LIMIT
///
/// MessageText:
///
///  Max number of established MM SAs to peer exceeded.
///
pub const ERROR_IPSEC_IKE_MM_LIMIT: i32 = 13882;

///
/// MessageId: ERROR_IPSEC_IKE_NEGOTIATION_DISABLED
///
/// MessageText:
///
///  IKE received a policy that disables negotiation.
///
pub const ERROR_IPSEC_IKE_NEGOTIATION_DISABLED: i32 = 13883;

///
/// MessageId: ERROR_IPSEC_IKE_NEG_STATUS_END
///
/// MessageText:
///
///  ERROR_IPSEC_IKE_NEG_STATUS_END
///
pub const ERROR_IPSEC_IKE_NEG_STATUS_END: i32 = 13884;
