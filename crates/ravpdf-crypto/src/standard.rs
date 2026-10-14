//! Standard Security Handler conforming to ISO 32000-1 Section 7.6.3.
//!
//! Traceability: FR-14.1 - FR-14.5, NFR-04.4, NFR-04.7.

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use aes::Aes128;
use md5::{Digest, Md5};
use ravpdf_core::error::{PdfError, Result};

type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes128CbcDec = cbc::Decryptor<Aes128>;

/// Standard 32-byte padding string specified in ISO 32000-1 Section 7.6.3.3.
pub const PDF_PADDING: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01,
    0x08, 0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53,
    0x69, 0x7A,
];

/// Pads or truncates a password string to exactly 32 bytes according to ISO 32000-1.
pub fn pad_password(password: &str) -> [u8; 32] {
    let mut buf = [0u8; 32];
    let bytes = password.as_bytes();
    if bytes.len() >= 32 {
        buf.copy_from_slice(&bytes[..32]);
    } else {
        buf[..bytes.len()].copy_from_slice(bytes);
        buf[bytes.len()..].copy_from_slice(&PDF_PADDING[..32 - bytes.len()]);
    }
    buf
}

/// Computes the 16-byte encryption key for Revision 2/3/4 standard security handler.
pub fn compute_encryption_key(
    password: &str,
    o_hash: &[u8],
    p_flags: i32,
    id_first: &[u8],
) -> [u8; 16] {
    let mut hasher = Md5::new();
    let padded = pad_password(password);
    hasher.update(&padded);
    hasher.update(o_hash);

    let p_bytes = p_flags.to_le_bytes();
    hasher.update(&p_bytes);
    hasher.update(id_first);

    let digest = hasher.finalize();
    let mut key = [0u8; 16];
    key.copy_from_slice(&digest);
    key
}

/// Encrypts data using AES-128-CBC with PKCS#7 padding.
/// The 16-byte IV is prepended to the ciphertext conforming to PDF specification.
pub fn encrypt_aes128(key: &[u8; 16], iv: &[u8; 16], plaintext: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(16 + plaintext.len() + 16);
    output.extend_from_slice(iv);

    let encryptor = Aes128CbcEnc::new(key.into(), iv.into());
    let mut buf = plaintext.to_vec();
    let block_len = 16;
    let pad_len = block_len - (plaintext.len() % block_len);
    buf.resize(plaintext.len() + pad_len, 0);

    let ct = encryptor
        .encrypt_padded_b2b_mut::<Pkcs7>(plaintext, &mut buf)
        .expect("encryption buffer sufficient");

    output.extend_from_slice(ct);
    output
}

/// Decrypts data where the first 16 bytes is the AES-128 initialization vector.
pub fn decrypt_aes128(key: &[u8; 16], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < 16 {
        return Err(PdfError::DecryptionError(
            "Ciphertext shorter than AES IV".into(),
        ));
    }

    let (iv, data) = ciphertext.split_at(16);
    let decryptor = Aes128CbcDec::new(key.into(), iv.into());

    let mut buf = vec![0u8; data.len()];
    let pt = decryptor
        .decrypt_padded_b2b_mut::<Pkcs7>(data, &mut buf)
        .map_err(|e| PdfError::DecryptionError(format!("AES-128 decryption failed: {:?}", e)))?;

    Ok(pt.to_vec())
}

/// Derives object-specific key for indirect object N G (ISO 32000-1 Section 7.6.2).
pub fn derive_object_key(doc_key: &[u8; 16], obj_num: u32, obj_gen: u16) -> [u8; 16] {
    let mut hasher = Md5::new();
    hasher.update(doc_key);
    hasher.update(&obj_num.to_le_bytes()[..3]);
    hasher.update(&obj_gen.to_le_bytes()[..2]);
    // For AES, append "sAlT" ASCII marker
    hasher.update(b"sAlT");

    let digest = hasher.finalize();
    let mut obj_key = [0u8; 16];
    obj_key.copy_from_slice(&digest);
    obj_key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes128_roundtrip() {
        let key = [0x42u8; 16];
        let iv = [0x11u8; 16];
        let message = b"Confidential document content encrypted with AES-128-CBC!";

        let ciphertext = encrypt_aes128(&key, &iv, message);
        let decrypted = decrypt_aes128(&key, &ciphertext).expect("decrypt aes");

        assert_eq!(message.to_vec(), decrypted);
    }
}
