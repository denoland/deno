// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ZeroCopyBuf;
use digest::{Digest, DynDigest};

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_create_hash", s.stateful_json_op2(op_create_hash));
  i.register_op("op_update_hash", s.stateful_json_op2(op_update_hash));
  i.register_op("op_digest_hash", s.stateful_json_op2(op_digest_hash));
}

struct HashAlgorithm {
  alg: Box<dyn DynDigest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HashCreateArgs {
  algorithm: String,
}

pub fn op_create_hash(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: HashCreateArgs = serde_json::from_value(args)?;

  let d: Box<HashAlgorithm> = match args.algorithm.as_str() {
    "md2" => Box::new(HashAlgorithm {
      alg: Box::new(md2::Md2::new()),
    }),
    "md4" => Box::new(HashAlgorithm {
      alg: Box::new(md4::Md4::new()),
    }),
    "md5" => Box::new(HashAlgorithm {
      alg: Box::new(md5::Md5::new()),
    }),
    "ripemd160" => Box::new(HashAlgorithm {
      alg: Box::new(ripemd160::Ripemd160::new()),
    }),
    "ripemd320" => Box::new(HashAlgorithm {
      alg: Box::new(ripemd320::Ripemd320::new()),
    }),
    "sha1" => Box::new(HashAlgorithm {
      alg: Box::new(sha1::Sha1::new()),
    }),
    "sha224" => Box::new(HashAlgorithm {
      alg: Box::new(sha2::Sha224::new()),
    }),
    "sha256" => Box::new(HashAlgorithm {
      alg: Box::new(sha2::Sha256::new()),
    }),
    "sha384" => Box::new(HashAlgorithm {
      alg: Box::new(sha2::Sha384::new()),
    }),
    "sha512" => Box::new(HashAlgorithm {
      alg: Box::new(sha2::Sha512::new()),
    }),
    "sha3-224" => Box::new(HashAlgorithm {
      alg: Box::new(sha3::Sha3_224::new()),
    }),
    "sha3-256" => Box::new(HashAlgorithm {
      alg: Box::new(sha3::Sha3_256::new()),
    }),
    "sha3-384" => Box::new(HashAlgorithm {
      alg: Box::new(sha3::Sha3_384::new()),
    }),
    "sha3-512" => Box::new(HashAlgorithm {
      alg: Box::new(sha3::Sha3_512::new()),
    }),
    "keccak224" => Box::new(HashAlgorithm {
      alg: Box::new(sha3::Keccak224::new()),
    }),
    "keccak256" => Box::new(HashAlgorithm {
      alg: Box::new(sha3::Keccak256::new()),
    }),
    "keccak384" => Box::new(HashAlgorithm {
      alg: Box::new(sha3::Keccak384::new()),
    }),
    "keccak512" => Box::new(HashAlgorithm {
      alg: Box::new(sha3::Keccak512::new()),
    }),
    _ => {
      return Err(OpError::type_error(format!(
        "Unknown hash algorithm: {}",
        args.algorithm
      )));
    }
  };

  let resource_table = isolate_state.resource_table.clone();
  {
    let mut resource_table = resource_table.borrow_mut();
    let rid = resource_table.add("hashDigest", d);
    Ok(JsonOp::Sync(json!(rid)))
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HashArgs {
  rid: u32,
}

pub fn op_update_hash(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: HashArgs = serde_json::from_value(args)?;

  // First we look up the rid in the resource table.
  let mut resource_table = isolate_state.resource_table.borrow_mut();
  let r = resource_table.get_mut::<HashAlgorithm>(args.rid);
  if let Some(hasher) = r {
    hasher.alg.input(&zero_copy[0]);
    Ok(JsonOp::Sync(json!(zero_copy[0].len())))
  } else {
    Err(OpError::bad_resource_id())
  }
}

pub fn op_digest_hash(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: HashArgs = serde_json::from_value(args)?;

  // First we look up the rid in the resource table.
  let resource_table = isolate_state.resource_table.borrow();
  let r = resource_table.get::<HashAlgorithm>(args.rid);
  if let Some(hasher) = r {
    let hash = hasher.alg.box_clone().result();
    Ok(JsonOp::Sync(json!(hash)))
  } else {
    Err(OpError::bad_resource_id())
  }
}
