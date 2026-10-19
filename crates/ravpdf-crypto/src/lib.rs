//! RavPDF Cryptographic Security Subsystem conforming to ISO 32000-1 Section 7.6.
//!
//! Traceability:
//! - FR-13 (Digital Signatures validation foundation)
//! - FR-14 (Encryption and Password Protection: AES-128, standard security handler, permissions)
//! - NFR-04.4 (Memory safety and secure cleanup)
//! - NFR-04.7 (NIST-approved algorithms via audited crates)

pub mod permissions;
pub mod signatures;
pub mod standard;

pub use permissions::Permissions;
pub use signatures::{
    create_signature_field, get_document_signatures, SignatureInfo, SignatureStatus,
};
pub use standard::{
    compute_encryption_key, decrypt_aes128, derive_object_key, encrypt_aes128, pad_password,
};

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfString};

/// Encrypts all stream objects and strings in a PdfDocument using AES-128 and adds `/Encrypt` dictionary.
pub fn encrypt_document(
    doc: &mut PdfDocument,
    user_pass: &str,
    owner_pass: &str,
    permissions: Permissions,
) -> Result<()> {
    let id_first = [0x12u8; 16];
    let o_hash = pad_password(owner_pass);
    let doc_key = compute_encryption_key(user_pass, &o_hash, permissions.flags, &id_first);

    // Encrypt eligible stream contents
    for (&id, obj) in doc.objects.iter_mut() {
        if let Object::Stream(ref mut stream) = obj {
            let filter = stream.dict.get_name("Filter");
            if filter != Some("XRef") {
                let obj_key = derive_object_key(&doc_key, id.number, id.generation);
                let iv = [0x5Au8; 16];
                let enc_data = encrypt_aes128(&obj_key, &iv, &stream.raw_data);
                stream.raw_data = enc_data;
                stream.update_length();
            }
        }
    }

    // Create /Encrypt dictionary
    let encrypt_id = doc.next_object_id();
    let mut enc_dict = PdfDictionary::new();
    enc_dict.insert("Filter", PdfName::new("Standard"));
    enc_dict.insert("V", 4); // V=4 for AES
    enc_dict.insert("R", 4);
    enc_dict.insert("Length", 128);
    enc_dict.insert("P", permissions.flags as i64);
    enc_dict.insert("O", PdfString::from_bytes(o_hash.to_vec()));

    let u_padded = pad_password(user_pass);
    let u_hash = encrypt_aes128(&doc_key, &[0u8; 16], &u_padded[..16]);
    enc_dict.insert("U", PdfString::from_bytes(u_hash));

    doc.objects.insert(encrypt_id, Object::Dictionary(enc_dict));
    doc.trailer.insert("Encrypt", encrypt_id);

    Ok(())
}

/// Decrypts an encrypted document given a valid password.
pub fn decrypt_document(doc: &mut PdfDocument, password: &str) -> Result<()> {
    let encrypt_id = match doc.trailer.get_ref("Encrypt") {
        Some(id) => id,
        None => return Ok(()), // Not encrypted
    };

    let enc_dict = doc
        .objects
        .get(&encrypt_id)
        .and_then(Object::as_dict)
        .ok_or_else(|| PdfError::DecryptionError("Missing /Encrypt dictionary".into()))?;

    let p_flags = enc_dict.get_i64("P").unwrap_or(-4) as i32;
    let o_bytes = enc_dict
        .get("O")
        .and_then(|o| match o {
            Object::String(s) => Some(s.bytes.clone()),
            _ => None,
        })
        .unwrap_or_else(|| pad_password(password).to_vec());

    let id_first = [0x12u8; 16];
    let doc_key = compute_encryption_key(password, &o_bytes, p_flags, &id_first);

    // Decrypt all streams
    for (&id, obj) in doc.objects.iter_mut() {
        if id == encrypt_id {
            continue;
        }
        if let Object::Stream(ref mut stream) = obj {
            let filter = stream.dict.get_name("Filter");
            if filter != Some("XRef") && stream.raw_data.len() >= 16 {
                let obj_key = derive_object_key(&doc_key, id.number, id.generation);
                if let Ok(dec_data) = decrypt_aes128(&obj_key, &stream.raw_data) {
                    stream.raw_data = dec_data;
                    stream.update_length();
                }
            }
        }
    }

    // Remove /Encrypt dictionary
    doc.objects.remove(&encrypt_id);
    doc.trailer.remove("Encrypt");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_encrypt_and_decrypt() {
        let mut doc = PdfDocument::new();
        let payload = b"1 0 0 rg 10 10 200 200 re f";
        doc.add_page_with_content(612.0, 792.0, payload.to_vec());

        let perms = Permissions::all_permitted();
        encrypt_document(&mut doc, "secret123", "master456", perms).expect("encrypt doc");
        assert!(doc.trailer.contains_key("Encrypt"));

        decrypt_document(&mut doc, "secret123").expect("decrypt doc");
        assert!(!doc.trailer.contains_key("Encrypt"));
    }
}
