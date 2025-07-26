// Copyright 2018-2025 the Deno authors. MIT license.

use std::ptr::NonNull;

use crate::ffi::Bio;
use crate::ffi::PKey;

#[derive(Debug)]
pub struct NetscapeSpki(*mut aws_lc_sys::NETSCAPE_SPKI);

impl NetscapeSpki {
  /// Decodes a base64-encoded SPKI certificate.
  fn from_base64(data: &[u8]) -> Result<Self, &'static str> {
    // Trim trailing characters for compatibility with OpenSSL.
    let end = data
      .iter()
      .rposition(|&b| !b" \n\r\t".contains(&b))
      .map_or(0, |i| i + 1);

    if end == 0 {
      return Err("Invalid SPKI data: no base64 content found");
    }

    // SAFETY: Cast data pointer to convert base64 to NETSCAPE_SPKI
    unsafe {
      let spki = aws_lc_sys::NETSCAPE_SPKI_b64_decode(
        data.as_ptr() as *const _,
        end as isize,
      );
      if spki.is_null() {
        return Err("Failed to decode base64 SPKI data");
      }
      Ok(NetscapeSpki(spki))
    }
  }

  fn verify(&self, pkey: &PKey) -> bool {
    // SAFETY: Use public key to verify SPKI certificate
    unsafe {
      let result = aws_lc_sys::NETSCAPE_SPKI_verify(self.0, pkey.as_ptr());
      result > 0
    }
  }

  fn spkac(&self) -> Result<&aws_lc_sys::NETSCAPE_SPKAC, &'static str> {
    // SAFETY: Access spkac field via raw pointer with null checks
    unsafe {
      if self.0.is_null() || (*self.0).spkac.is_null() {
        return Err("Invalid SPKAC structure");
      }
      Ok(&*(*self.0).spkac)
    }
  }

  fn get_public_key(&self) -> Result<PKey, &'static str> {
    // SAFETY: Extract public key, null checked by PKey::from_ptr
    unsafe {
      let pkey = aws_lc_sys::NETSCAPE_SPKI_get_pubkey(self.0);
      PKey::from_ptr(pkey).ok_or("Failed to extract public key")
    }
  }

  fn get_challenge(&self) -> Result<Vec<u8>, &'static str> {
    // SAFETY: Extract challenge with null checks and BufferGuard for cleanup
    unsafe {
      let spkac = self.spkac()?;
      let challenge = spkac.challenge;
      if challenge.is_null() {
        return Err("No challenge found in SPKI certificate");
      }

      let mut buf = std::ptr::null_mut();
      let buf_len = aws_lc_sys::ASN1_STRING_to_UTF8(&mut buf, challenge);

      if buf_len <= 0 || buf.is_null() {
        return Err("Failed to extract challenge string");
      }

      let _guard = BufferGuard(NonNull::new(buf).unwrap());

      let challenge_slice =
        std::slice::from_raw_parts(buf as *const u8, buf_len as usize);
      Ok(challenge_slice.to_vec())
    }
  }

  pub fn as_ptr(&self) -> *mut aws_lc_sys::NETSCAPE_SPKI {
    self.0
  }
}

impl Drop for NetscapeSpki {
  fn drop(&mut self) {
    // SAFETY: Free NETSCAPE_SPKI with null check
    unsafe {
      if !self.0.is_null() {
        aws_lc_sys::NETSCAPE_SPKI_free(self.0);
      }
    }
  }
}

// RAII guard for automatically freeing ASN1 string buffers
struct BufferGuard(NonNull<u8>);

impl Drop for BufferGuard {
  fn drop(&mut self) {
    // SAFETY: Free ASN1_STRING buffer (NonNull guarantees non-null)
    unsafe {
      aws_lc_sys::OPENSSL_free(self.0.as_ptr() as *mut std::ffi::c_void);
    }
  }
}

/// Validates the SPKAC data structure.
///
/// Returns true if the signature in the SPKAC data is valid.
pub fn verify_spkac(data: &[u8]) -> bool {
  let spki = match NetscapeSpki::from_base64(data) {
    Ok(spki) => spki,
    Err(_) => return false,
  };

  let pkey = match extract_public_key_from_spkac(&spki) {
    Ok(pkey) => pkey,
    Err(_) => return false,
  };

  spki.verify(&pkey)
}

/// Extracts the public key from the SPKAC structure.
fn extract_public_key_from_spkac(
  spki: &NetscapeSpki,
) -> Result<PKey, &'static str> {
  // SAFETY: Extract public key with null checks and proper ownership
  unsafe {
    let spkac = spki.spkac()?;
    let pubkey = spkac.pubkey;
    if pubkey.is_null() {
      return Err("No public key in SPKAC structure");
    }

    let pkey = aws_lc_sys::X509_PUBKEY_get(pubkey);
    PKey::from_ptr(pkey).ok_or("Failed to extract public key from X509_PUBKEY")
  }
}

/// Exports the public key from the SPKAC data in PEM format.
pub fn export_public_key(data: &[u8]) -> Option<Vec<u8>> {
  let spki = NetscapeSpki::from_base64(data).ok()?;

  let pkey = spki.get_public_key().ok()?;

  let bio = Bio::new_memory().ok()?;
  // SAFETY: Write public key to BIO in PEM format, check result
  unsafe {
    let result = aws_lc_sys::PEM_write_bio_PUBKEY(bio.as_ptr(), pkey.as_ptr());
    if result <= 0 {
      return None;
    }
  }

  bio.get_contents().ok()
}

/// Exports the challenge string from the SPKAC data.
pub fn export_challenge(data: &[u8]) -> Option<Vec<u8>> {
  let spki = NetscapeSpki::from_base64(data).ok()?;

  spki.get_challenge().ok()
}

#[cfg(test)]
mod tests {
  use crate::spki::verify_spkac;

  #[test]
  fn test_md_spkac() {
    // md4 and md5 based signatures are not supported.
    // https://github.com/aws/aws-lc/commit/7e28b9ee89d85fbc80b69bc0eeb0070de81ac563
    let spkac_data = br#"MIICUzCCATswggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC33FiIiiexwLe/P8DZx5HsqFlmUO7/lvJ7necJVNwqdZ3ax5jpQB0p6uxfqeOvzcN3k5V7UFb/Am+nkSNZMAZhsWzCU2Z4Pjh50QYz3f0Hour7/yIGStOLyYY3hgLK2K8TbhgjQPhdkw9+QtKlpvbL8fLgONAoGrVOFnRQGcr70iFffsm79mgZhKVMgYiHPJqJgGHvCtkGg9zMgS7p63+Q3ZWedtFS2RhMX3uCBy/mH6EOlRCNBbRmA4xxNzyf5GQaki3T+Iz9tOMjdPP+CwV2LqEdylmBuik8vrfTb3qIHLKKBAI8lXN26wWtA3kN4L7NP+cbKlCRlqctvhmylLH1AgMBAAEWE3RoaXMtaXMtYS1jaGFsbGVuZ2UwDQYJKoZIhvcNAQEEBQADggEBAIozmeW1kfDfAVwRQKileZGLRGCD7AjdHLYEe16xTBPve8Af1bDOyuWsAm4qQLYA4FAFROiKeGqxCtIErEvm87/09tCfF1My/1Uj+INjAk39DK9J9alLlTsrwSgd1lb3YlXY7TyitCmh7iXLo4pVhA2chNA3njiMq3CUpSvGbpzrESL2dv97lv590gUD988wkTDVyYsf0T8+X0Kww3AgPWGji+2f2i5/jTfD/s1lK1nqi7ZxFm0pGZoy1MJ51SCEy7Y82ajroI+5786nC02mo9ak7samca4YDZOoxN4d3tax4B/HDF5dqJSm1/31xYLDTfujCM5FkSjRc4m6hnriEkc="#;

    assert!(!verify_spkac(spkac_data));
  }

  #[test]
  fn test_spkac_verify() {
    let spkac = b"MIICUzCCATswggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCXzfKgGnkkOF7+VwMzGpiWy5nna/VGJOfPBsCVg5WooJHN9nAFyqLxoV0WyhwvIdHhIgcTX2L4BHRa+4B0zb4stRHK02ZknJvionK4kBfa+k7Q4DzasW3ulLCTXPLVBKzW9QSzE4Wult17BX6uSUy3Bpr/Nuk6B4Ja3JnFpdSYmJbWP55kRONFBZYPCXr7T8k6hzEHcevFE/PUi6IU+LKiwyGH5KXAUzRbMtqbZLn/rEAmEBxmv/z/+shAwiRE8s9RqBi+pVdwqWdw6ibNkbM7G3j4CMyfAk7EOpGf5loRIrVWB4XrVYWb2EQ6sd9LfiQ9GwqlFYw006MUo6nxoEtNAgMBAAEWE3RoaXMtaXMtYS1jaGFsbGVuZ2UwDQYJKoZIhvcNAQELBQADggEBAHUw1UoZjG7TCb/JhFo5p8XIFeizGEwYoqttBoVTQ+MeCfnNoLLeAyId0atb2jPnYsI25Z/PHHV1N9t0L/NelY3rZC/Z00Wx8IGeslnGXXbqwnp36Umb0r2VmxTr8z1QaToGyOQXp4Xor9qbQFoANIivyVUYsuqJ1FnDJCC/jBPo4IWiQbTst331v2fiVdV+/XUh9AIjcm4085b65HjFwLxDeWhbgAZ+UfhqBbTVA1K8uUqS8e3gbeaNstZvnclxZ3PlHSk8v1RdIG4e5ThTOwPH5u/7KKeafn9SwgY/Q8KqaVfHHCv1IeVlijamjnyFhWc35kGlBUNgLOnWAOE3GsM=";
    assert!(verify_spkac(spkac));
  }

  #[test]
  fn test_spkac_empty() {
    let empty_spkac = b"";
    assert!(!verify_spkac(empty_spkac));
  }
}
