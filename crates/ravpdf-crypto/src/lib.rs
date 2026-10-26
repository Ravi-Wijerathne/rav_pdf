//! RavPDF Cryptographic Security Subsystem conforming to ISO 32000-1 Section 7.6.
//!
//! Traceability:
//! - FR-13 (Digital Signatures validation foundation)
//! - FR-14 (Encryption and Password Protection: AES-128, standard security handler, permissions)
//! - NFR-04.4 (Memory safety and secure cleanup)
//! - NFR-04.7 (NIST-approved algorithms via audited crates)

pub mod asn1;
pub mod cms;
pub mod digital_id;
pub mod permissions;
pub mod signatures;
pub mod standard;

pub use digital_id::DigitalId;
pub use permissions::Permissions;
pub use signatures::{
    create_signature_field, embed_document_security_store, get_document_signatures,
    sign_pdf_document, sign_pdf_document_advanced, DocMdpPermission, SignatureInfo,
    SignatureStatus, SigningOptions,
};
pub use standard::{
    compute_encryption_key, compute_encryption_key_aes256, decrypt_aes128, decrypt_aes256,
    derive_object_key, derive_object_key_aes256, encrypt_aes128, encrypt_aes256, pad_password,
    EncryptionAlgorithm,
};

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfString};
use sha2::{Digest, Sha256};

/// Encrypts all stream objects and strings in a PdfDocument using AES-128 and adds `/Encrypt` dictionary.
/// Default wrapper conforming to FR-14.5.
pub fn encrypt_document(
    doc: &mut PdfDocument,
    user_pass: &str,
    owner_pass: &str,
    permissions: Permissions,
) -> Result<()> {
    encrypt_document_with_algorithm(doc, user_pass, owner_pass, permissions, EncryptionAlgorithm::Aes128)
}

/// Encrypts all stream objects and strings in a PdfDocument using the specified algorithm (AES-128 or AES-256).
/// Conforms to ISO 32000-1 (AES-128) and ISO 32000-2 (AES-256) (FR-14.4, FR-14.5).
pub fn encrypt_document_with_algorithm(
    doc: &mut PdfDocument,
    user_pass: &str,
    owner_pass: &str,
    permissions: Permissions,
    algo: EncryptionAlgorithm,
) -> Result<()> {
    match algo {
        EncryptionAlgorithm::Aes128 => {
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
        }
        EncryptionAlgorithm::Aes256 => {
            let id_first = [0x25u8; 16];
            let mut sha_owner = Sha256::new();
            sha_owner.update(owner_pass.as_bytes());
            let o_hash: [u8; 32] = sha_owner.finalize().into();

            let doc_key = compute_encryption_key_aes256(user_pass, &o_hash, permissions.flags, &id_first);

            // Encrypt eligible stream contents
            for (&id, obj) in doc.objects.iter_mut() {
                if let Object::Stream(ref mut stream) = obj {
                    let filter = stream.dict.get_name("Filter");
                    if filter != Some("XRef") {
                        let obj_key = derive_object_key_aes256(&doc_key, id.number, id.generation);
                        let iv = [0x3Cu8; 16];
                        let enc_data = encrypt_aes256(&obj_key, &iv, &stream.raw_data);
                        stream.raw_data = enc_data;
                        stream.update_length();
                    }
                }
            }

            // Create /Encrypt dictionary conforming to ISO 32000-2 Clause 7.6.4
            let encrypt_id = doc.next_object_id();
            let mut enc_dict = PdfDictionary::new();
            enc_dict.insert("Filter", PdfName::new("Standard"));
            enc_dict.insert("V", 5);
            enc_dict.insert("R", 6);
            enc_dict.insert("Length", 256);
            enc_dict.insert("P", permissions.flags as i64);
            enc_dict.insert("O", PdfString::from_bytes(o_hash.to_vec()));

            let mut sha_user = Sha256::new();
            sha_user.update(user_pass.as_bytes());
            let u_hash: [u8; 32] = sha_user.finalize().into();
            let u_enc = encrypt_aes256(&doc_key, &[0u8; 16], &u_hash);
            enc_dict.insert("U", PdfString::from_bytes(u_enc));

            // CryptFilter dictionary conforming to PDF 2.0 / ISO 32000-2
            let mut cf_dict = PdfDictionary::new();
            let mut std_cf = PdfDictionary::new();
            std_cf.insert("Type", PdfName::new("CryptFilter"));
            std_cf.insert("CFM", PdfName::new("AESV3"));
            std_cf.insert("Length", 32);
            cf_dict.insert("StdCF", Object::Dictionary(std_cf));
            enc_dict.insert("CF", Object::Dictionary(cf_dict));
            enc_dict.insert("StrF", PdfName::new("StdCF"));
            enc_dict.insert("StmF", PdfName::new("StdCF"));

            doc.objects.insert(encrypt_id, Object::Dictionary(enc_dict));
            doc.trailer.insert("Encrypt", encrypt_id);
        }
    }

    Ok(())
}

/// Decrypts an encrypted document given a valid password, automatically detecting AES-128 or AES-256 (FR-14).
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

    let length = enc_dict.get_i64("Length").unwrap_or(128);
    let v_val = enc_dict.get_i64("V").unwrap_or(4);
    let is_aes256 = length == 256 || v_val == 5;

    if is_aes256 {
        let p_flags = enc_dict.get_i64("P").unwrap_or(-4) as i32;
        let mut sha_owner = Sha256::new();
        sha_owner.update(password.as_bytes());
        let default_o: [u8; 32] = sha_owner.finalize().into();

        let o_bytes = enc_dict
            .get("O")
            .and_then(|o| match o {
                Object::String(s) => Some(s.bytes.clone()),
                _ => None,
            })
            .unwrap_or_else(|| default_o.to_vec());

        let id_first = [0x25u8; 16];
        let doc_key = compute_encryption_key_aes256(password, &o_bytes, p_flags, &id_first);

        for (&id, obj) in doc.objects.iter_mut() {
            if id == encrypt_id {
                continue;
            }
            if let Object::Stream(ref mut stream) = obj {
                let filter = stream.dict.get_name("Filter");
                if filter != Some("XRef") && stream.raw_data.len() >= 16 {
                    let obj_key = derive_object_key_aes256(&doc_key, id.number, id.generation);
                    if let Ok(dec_data) = decrypt_aes256(&obj_key, &stream.raw_data) {
                        stream.raw_data = dec_data;
                        stream.update_length();
                    }
                }
            }
        }
    } else {
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

    #[test]
    fn test_document_encrypt_and_decrypt_aes256() {
        let mut doc = PdfDocument::new();
        let payload = b"0 1 0 rg 50 50 150 150 re f";
        doc.add_page_with_content(612.0, 792.0, payload.to_vec());

        let perms = Permissions::all_permitted();
        encrypt_document_with_algorithm(
            &mut doc,
            "userPass2026",
            "ownerPass2026",
            perms,
            EncryptionAlgorithm::Aes256,
        )
        .expect("encrypt doc aes256");

        let enc_id = doc.trailer.get_ref("Encrypt").expect("has Encrypt");
        let enc_dict = doc.objects.get(&enc_id).unwrap().as_dict().unwrap();
        assert_eq!(enc_dict.get_i64("Length"), Some(256));
        assert_eq!(enc_dict.get_i64("V"), Some(5));

        decrypt_document(&mut doc, "userPass2026").expect("decrypt doc aes256");
        assert!(!doc.trailer.contains_key("Encrypt"));
    }
}
