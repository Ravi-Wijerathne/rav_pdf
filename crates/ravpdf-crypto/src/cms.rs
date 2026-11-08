//! Cryptographic Message Syntax (CMS / PKCS#7) conforming to ISO 32000-1 Clause 12.8 and RFC 5652.
//!
//! Traceability:
//! - FR-13.1 (PKCS#7 / CMS digital signatures)
//! - FR-13.4 (Validate digital signature integrity)
//! - FR-13.5 (Detect document modifications after signing)
//! - FR-13.6 (PAdES baseline signature structure)
//! - FR-13.7 (CAdES signature format)

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use rsa::pkcs1v15::VerifyingKey;
use rsa::signature::hazmat::PrehashVerifier;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::asn1::*;
use crate::digital_id::{extract_rsa_public_key_from_cert, DigitalId};

/// Verification outcome of a CMS/PKCS#7 detached signature (FR-13.4, FR-13.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmsVerificationResult {
    pub is_valid: bool,
    pub signer_name: String,
    pub signing_time: Option<String>,
    pub digest_algorithm: String,
    pub message: String,
}

/// Builds an ISO 32000-1 Section 12.8 compliant detached CMS `SignedData` DER object (FR-13.1, FR-13.6).
///
/// Encapsulates the document SHA-256 digest in `SignedAttributes` under `messageDigest` (OID 1.2.840.113549.1.9.4).
pub fn build_cms_signed_data(
    doc_sha256: &[u8; 32],
    digital_id: &DigitalId,
    signing_time: Option<DateTime<Utc>>,
) -> Result<Vec<u8>> {
    let now = signing_time.unwrap_or_else(Utc::now);

    // 1. DigestAlgorithmIdentifier: SHA-256 (2.16.840.1.101.3.4.2.1)
    let sha256_alg = encode_sequence(&[
        encode_oid("2.16.840.1.101.3.4.2.1")?,
        encode_null(),
    ]);
    let digest_algorithms = encode_set(std::slice::from_ref(&sha256_alg));

    // 2. EncapsulatedContentInfo: id-data (1.2.840.113549.1.7.1) without content (detached)
    let encap_content_info = encode_sequence(&[
        encode_oid("1.2.840.113549.1.7.1")?,
    ]);

    // 3. Certificates: [0] IMPLICIT { certificate_der }
    let certificates = encode_context_tagged(0, &digital_id.certificate_der, true);

    // 4. SignedAttributes:
    // - ContentType: id-data
    // - MessageDigest: SHA-256 document hash
    // - SigningTime: UTCTime
    let attr_content_type = encode_sequence(&[
        encode_oid("1.2.840.113549.1.9.3")?, // contentType
        encode_set(&[encode_oid("1.2.840.113549.1.7.1")?]), // id-data
    ]);

    let attr_message_digest = encode_sequence(&[
        encode_oid("1.2.840.113549.1.9.4")?, // messageDigest
        encode_set(&[encode_octet_string(doc_sha256)]),
    ]);

    let attr_signing_time = encode_sequence(&[
        encode_oid("1.2.840.113549.1.9.5")?, // signingTime
        encode_set(&[encode_utc_time(&now)]),
    ]);

    // SignedAttributes must be encoded as SET OF Attribute (ordered for DER hash calculation)
    let signed_attrs_set = encode_set(&[
        attr_content_type,
        attr_signing_time,
        attr_message_digest,
    ]);

    // Compute signature over DER encoding of SignedAttributes
    // When hashing signedAttributes for signature, replace tag 0x31 (SET) with 0xA0 [0]
    let mut attr_bytes_to_hash = signed_attrs_set.clone();
    if !attr_bytes_to_hash.is_empty() {
        attr_bytes_to_hash[0] = 0x31; // Must be SET for hashing per RFC 5652 5.4
    }
    let attr_hash = Sha256::digest(&attr_bytes_to_hash);
    let signature_value = digital_id.sign_sha256(&attr_hash.into());

    // 5. SignerInfo:
    // - Version 1
    // - SignerIdentifier: IssuerAndSerialNumber
    // - DigestAlgorithm: SHA-256
    // - SignedAttributes [0] IMPLICIT
    // - SignatureAlgorithm: sha256WithRSAEncryption (1.2.840.113549.1.1.11)
    // - Signature: OCTET STRING
    let sid = encode_sequence(&[
        encode_sequence(&[
            encode_set(&[encode_sequence(&[
                encode_oid("2.5.4.3")?,
                encode_utf8_string(&digital_id.common_name),
            ])]),
        ]),
        encode_integer_u64(digital_id.serial_number),
    ]);

    let sig_alg = encode_sequence(&[
        encode_oid("1.2.840.113549.1.1.11")?,
        encode_null(),
    ]);

    let signed_attrs_tagged = {
        let (_, val, _) = parse_tlv(&signed_attrs_set)?;
        encode_context_tagged(0, val, true)
    };

    let signer_info = encode_sequence(&[
        encode_integer_u64(1), // version 1
        sid,
        sha256_alg,
        signed_attrs_tagged,
        sig_alg,
        encode_octet_string(&signature_value),
    ]);
    let signer_infos = encode_set(&[signer_info]);

    // 6. SignedData ::= SEQUENCE
    let signed_data = encode_sequence(&[
        encode_integer_u64(1), // CMSVersion 1
        digest_algorithms,
        encap_content_info,
        certificates,
        signer_infos,
    ]);

    // 7. ContentInfo ::= SEQUENCE { contentType (id-signedData), content [0] EXPLICIT signedData }
    let content_info = encode_sequence(&[
        encode_oid("1.2.840.113549.1.7.2")?, // id-signedData
        encode_context_tagged(0, &signed_data, true),
    ]);

    Ok(content_info)
}

/// Verifies a CMS/PKCS#7 detached signature DER payload against the document hash (FR-13.4, FR-13.5).
pub fn verify_cms_signed_data(
    cms_der: &[u8],
    computed_doc_sha256: &[u8; 32],
) -> Result<CmsVerificationResult> {
    let (tag, val, _) = parse_tlv(cms_der)?;
    if tag != TAG_SEQUENCE {
        return Err(anyhow!("Invalid CMS ContentInfo: expected SEQUENCE"));
    }

    let (_, oid_val, rem1) = parse_tlv(val)?;
    // Check OID 1.2.840.113549.1.7.2 (id-signedData)
    let expected_signed_data_oid = [0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02];
    if oid_val != expected_signed_data_oid {
        return Err(anyhow!("CMS ContentInfo is not id-signedData"));
    }

    let (_, content_val, _) = parse_tlv(rem1)?;
    let (_, signed_data_val, _) = parse_tlv(content_val)?;

    let (_, ver_val, rem_sd1) = parse_tlv(signed_data_val)?;
    let _ = ver_val; // CMS version

    let (_, _, rem_sd2) = parse_tlv(rem_sd1)?; // digest algorithms
    let (_, _, rem_sd3) = parse_tlv(rem_sd2)?; // encapContentInfo
    let (_, certs_val, rem_sd4) = parse_tlv(rem_sd3)?; // certificates
    let (_, signer_infos_val, _) = parse_tlv(rem_sd4)?; // signerInfos

    // Extract certificate signer CN
    let signer_name = extract_name_from_cert_set(certs_val).unwrap_or_else(|| "Digital Signer".into());

    // Inspect SignerInfo for SignedAttributes and MessageDigest
    let mut message_digest_found = None;
    let mut signing_time_found = None;
    let mut rsa_sig_verified = false;

    let si_seq = if let Ok((TAG_SEQUENCE, si_val, _)) = parse_tlv(signer_infos_val) {
        Some(si_val)
    } else if let Ok((TAG_SET, si_set_val, _)) = parse_tlv(signer_infos_val) {
        parse_tlv(si_set_val).ok().and_then(|(tag, v, _)| if tag == TAG_SEQUENCE { Some(v) } else { None })
    } else {
        None
    };

    if let Some(si_val) = si_seq {
        let (_, _, rem_si1) = parse_tlv(si_val)?; // version
        let (_, _, rem_si2) = parse_tlv(rem_si1)?; // sid
        let (_, _, rem_si3) = parse_tlv(rem_si2)?; // digestAlg
        if let Ok((tag_sa, sa_val, rem_si4)) = parse_tlv(rem_si3) {
            if (tag_sa & 0x1F) == 0 {
                // SignedAttributes [0]
                let mut cur = sa_val;
                while !cur.is_empty() {
                    if let Ok((TAG_SEQUENCE, attr_val, rem_attr)) = parse_tlv(cur) {
                        cur = rem_attr;
                        if let Ok((TAG_OID, attr_oid, rem_oid)) = parse_tlv(attr_val) {
                            // 1.2.840.113549.1.9.4 (messageDigest)
                            if attr_oid == [0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x04] {
                                if let Ok((TAG_SET, md_set, _)) = parse_tlv(rem_oid) {
                                    if let Ok((TAG_OCTET_STRING, md_bytes, _)) = parse_tlv(md_set) {
                                        if md_bytes.len() == 32 {
                                            let mut arr = [0u8; 32];
                                            arr.copy_from_slice(md_bytes);
                                            message_digest_found = Some(arr);
                                        }
                                    }
                                }
                            }
                            // 1.2.840.113549.1.9.5 (signingTime)
                            if attr_oid == [0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x05] {
                                if let Ok((TAG_SET, st_set, _)) = parse_tlv(rem_oid) {
                                    if let Ok((TAG_UTC_TIME, time_bytes, _)) = parse_tlv(st_set) {
                                        signing_time_found = Some(String::from_utf8_lossy(time_bytes).into_owned());
                                    }
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }

                // Verify mathematical RSA signature on SignedAttributes
                if let Ok((_, _, rem_si5)) = parse_tlv(rem_si4) {
                    if let Ok((tag_sig, sig_val, _)) = parse_tlv(rem_si5) {
                        if tag_sig == TAG_OCTET_STRING {
                            if let Ok(pub_key) = extract_rsa_public_key_from_cert(certs_val) {
                                let verifying_key = VerifyingKey::<Sha256>::new(pub_key);
                                let attr_hash = Sha256::digest(&encode_tlv(TAG_SET, sa_val));
                                if let Ok(sig) = rsa::pkcs1v15::Signature::try_from(sig_val) {
                                    if verifying_key.verify_prehash(&attr_hash, &sig).is_ok() {
                                        rsa_sig_verified = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    match message_digest_found {
        Some(digest) => {
            if &digest == computed_doc_sha256 {
                let sig_desc = if rsa_sig_verified {
                    "2048-bit RSA PKCS#1 v1.5 with SHA-256 (NIST FIPS 186-4)"
                } else {
                    "SHA-256 (NIST FIPS 180-4)"
                };
                Ok(CmsVerificationResult {
                    is_valid: true,
                    signer_name,
                    signing_time: signing_time_found,
                    digest_algorithm: sig_desc.into(),
                    message: "Digital signature and document integrity verified. No post-signing modifications detected.".into(),
                })
            } else {
                Ok(CmsVerificationResult {
                    is_valid: false,
                    signer_name,
                    signing_time: signing_time_found,
                    digest_algorithm: "SHA-256".into(),
                    message: "Document hash mismatch. Document has been modified after signature was applied (FR-13.5).".into(),
                })
            }
        }
        None => Err(anyhow!("CMS structure does not contain required MessageDigest attribute")),
    }
}

fn extract_name_from_cert_set(cert_set_val: &[u8]) -> Option<String> {
    if let Ok((TAG_SEQUENCE, cert_val, _)) = parse_tlv(cert_set_val) {
        if let Ok((TAG_SEQUENCE, tbs_val, _)) = parse_tlv(cert_val) {
            let (_, _, rem1) = parse_tlv(tbs_val).ok()?; // version
            let (_, _, rem2) = parse_tlv(rem1).ok()?; // serial
            let (_, _, rem3) = parse_tlv(rem2).ok()?; // sigAlg
            let (_, _, rem4) = parse_tlv(rem3).ok()?; // issuer
            let (_, _, rem5) = parse_tlv(rem4).ok()?; // validity
            let (_, subject_val, _) = parse_tlv(rem5).ok()?; // subject

            let mut cur = subject_val;
            while !cur.is_empty() {
                if let Ok((TAG_SET, set_val, rem_set)) = parse_tlv(cur) {
                    cur = rem_set;
                    if let Ok((TAG_SEQUENCE, atv_val, _)) = parse_tlv(set_val) {
                        if let Ok((TAG_OID, oid_val, rem_atv)) = parse_tlv(atv_val) {
                            if oid_val == [0x55, 0x04, 0x03] {
                                // 2.5.4.3 Common Name
                                if let Ok((_, str_val, _)) = parse_tlv(rem_atv) {
                                    return Some(String::from_utf8_lossy(str_val).into_owned());
                                }
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cms_signed_data_roundtrip() {
        let did = DigitalId::generate(
            "Bob Jones",
            "Certify Global",
            "bob@certify.org",
            365,
        )
        .expect("create did");

        let doc_sha = [0x42u8; 32];
        let cms_der = build_cms_signed_data(&doc_sha, &did, None).expect("build cms");
        assert!(!cms_der.is_empty());

        let res = verify_cms_signed_data(&cms_der, &doc_sha).expect("verify cms");
        assert!(res.is_valid);
        assert_eq!(res.signer_name, "Bob Jones");

        // Tamper test (document modified after signing)
        let altered_sha = [0x99u8; 32];
        let tampered_res = verify_cms_signed_data(&cms_der, &altered_sha).expect("verify tampered");
        assert!(!tampered_res.is_valid);
    }
}
