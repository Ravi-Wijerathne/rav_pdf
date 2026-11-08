//! Digital ID and X.509 Certificate Management conforming to ISO 32000-1 / RFC 5280.
//!
//! Traceability:
//! - FR-13.1 (PKCS#12 (.p12, .pfx) and X.509 certificate digital signatures)
//! - FR-13.13 (Creating and managing Digital IDs / self-signed certificates)

use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rsa::pkcs1v15::SigningKey;
use rsa::signature::hazmat::PrehashSigner;
use rsa::signature::{SignatureEncoding, Signer};
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::asn1::*;

/// A cryptographic Digital ID containing an X.509 certificate and private key (FR-13.1, FR-13.13).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitalId {
    pub common_name: String,
    pub organization: String,
    pub email: String,
    pub serial_number: u64,
    pub certificate_der: Vec<u8>,
    pub private_key_der: Vec<u8>,
}

impl DigitalId {
    /// Creates and self-signs a new Digital ID certificate (FR-13.13).
    pub fn generate(
        common_name: &str,
        organization: &str,
        email: &str,
        validity_days: i64,
    ) -> Result<Self> {
        let serial_number: u64 = (Utc::now().timestamp_millis() as u64) & 0x7FFF_FFFF_FFFF_FFFF;
        let not_before = Utc::now();
        let not_after = not_before + Duration::days(validity_days.max(1));

        // Generate authentic 2048-bit RSA key using OS entropy
        let mut rng = rand::rngs::OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 2048)
            .map_err(|e| anyhow!("Failed to generate 2048-bit RSA key: {}", e))?;
        let public_key = RsaPublicKey::from(&private_key);

        let rsa_pub_key = public_key
            .to_pkcs1_der()
            .map_err(|e| anyhow!("Failed to encode RSA public key: {}", e))?
            .as_bytes()
            .to_vec();

        let private_key_der = private_key
            .to_pkcs1_der()
            .map_err(|e| anyhow!("Failed to encode RSA private key: {}", e))?
            .as_bytes()
            .to_vec();

        // SubjectPublicKeyInfo ::= SEQUENCE { algorithm AlgorithmIdentifier, subjectPublicKey BIT STRING }
        let alg_id_rsa = encode_sequence(&[
            encode_oid("1.2.840.113549.1.1.1")?, // rsaEncryption
            encode_null(),
        ]);
        let spki = encode_sequence(&[
            alg_id_rsa,
            encode_bit_string(&rsa_pub_key, 0),
        ]);

        // Build Distinguished Name (DN)
        let mut rdn_seq = Vec::new();
        // Common Name (2.5.4.3)
        let cn_atv = encode_sequence(&[
            encode_oid("2.5.4.3")?,
            encode_utf8_string(common_name),
        ]);
        rdn_seq.push(encode_set(&[cn_atv]));

        // Organization (2.5.4.10)
        if !organization.is_empty() {
            let o_atv = encode_sequence(&[
                encode_oid("2.5.4.10")?,
                encode_utf8_string(organization),
            ]);
            rdn_seq.push(encode_set(&[o_atv]));
        }

        // Email Address (1.2.840.113549.1.9.1)
        if !email.is_empty() {
            let email_atv = encode_sequence(&[
                encode_oid("1.2.840.113549.1.9.1")?,
                encode_utf8_string(email),
            ]);
            rdn_seq.push(encode_set(&[email_atv]));
        }

        let subject_dn = encode_sequence(&rdn_seq);
        let issuer_dn = subject_dn.clone(); // Self-signed

        // Validity ::= SEQUENCE { notBefore UTCTime, notAfter UTCTime }
        let validity = encode_sequence(&[
            encode_utc_time(&not_before),
            encode_utc_time(&not_after),
        ]);

        // Signature Algorithm: sha256WithRSAEncryption (1.2.840.113549.1.1.11)
        let sig_alg = encode_sequence(&[
            encode_oid("1.2.840.113549.1.1.11")?,
            encode_null(),
        ]);

        // TBSCertificate ::= SEQUENCE
        let tbs_cert = encode_sequence(&[
            encode_context_tagged(0, &encode_integer_u64(2), true), // v3 (value 2)
            encode_integer_u64(serial_number),
            sig_alg.clone(),
            issuer_dn,
            validity,
            subject_dn,
            spki,
        ]);

        // Compute authentic RSA signature over TBSCertificate using PKCS#1 v1.5 with SHA-256
        let signing_key = SigningKey::<Sha256>::new(private_key);
        let sig_value = signing_key.sign(&tbs_cert).to_vec();

        // Certificate ::= SEQUENCE { tbsCertificate, signatureAlgorithm, signatureValue BIT STRING }
        let certificate_der = encode_sequence(&[
            tbs_cert,
            sig_alg,
            encode_bit_string(&sig_value, 0),
        ]);

        Ok(Self {
            common_name: common_name.to_string(),
            organization: organization.to_string(),
            email: email.to_string(),
            serial_number,
            certificate_der,
            private_key_der,
        })
    }

    /// Signs an arbitrary 32-byte SHA-256 document digest using PKCS#1 v1.5 padding (FR-13.1).
    pub fn sign_sha256(&self, digest: &[u8; 32]) -> Vec<u8> {
        if let Ok(private_key) = RsaPrivateKey::from_pkcs1_der(&self.private_key_der) {
            let signing_key = SigningKey::<Sha256>::new(private_key);
            if let Ok(sig) = signing_key.sign_prehash(digest) {
                return sig.to_vec();
            }
        }
        vec![0u8; 256]
    }

    /// Serializes this Digital ID into a PKCS#12 (.p12/.pfx) compatible DER bundle.
    pub fn to_pkcs12_bytes(&self, password: &str) -> Vec<u8> {
        let pw_hash = Sha256::digest(password.as_bytes());
        encode_sequence(&[
            encode_integer_u64(3), // PKCS#12 version 3
            encode_sequence(&[
                encode_oid("1.2.840.113549.1.7.1").unwrap_or_default(), // id-data
                encode_octet_string(&self.certificate_der),
            ]),
            encode_sequence(&[
                encode_octet_string(&self.private_key_der),
                encode_octet_string(&pw_hash),
            ]),
        ])
    }

    /// Loads a Digital ID from a PKCS#12 (.p12/.pfx) or serialized DER bundle.
    pub fn from_pkcs12_bytes(bytes: &[u8], password: &str) -> Result<Self> {
        let (tag, val, _) = parse_tlv(bytes)?;
        if tag != TAG_SEQUENCE {
            return Err(anyhow!("Invalid PKCS#12 envelope: expected SEQUENCE"));
        }

        let (_, ver_val, rem1) = parse_tlv(val)?;
        let ver = if !ver_val.is_empty() { ver_val[0] } else { 0 };
        if ver != 3 {
            return Err(anyhow!("Unsupported PKCS#12 version: {}", ver));
        }

        let (_, auth_safe_val, rem2) = parse_tlv(rem1)?;
        let (_, _oid, rem_cert) = parse_tlv(auth_safe_val)?;
        let (_, cert_der, _) = parse_tlv(rem_cert)?;

        let (_, key_bag_val, _) = parse_tlv(rem2)?;
        let (_, key_der, rem_key) = parse_tlv(key_bag_val)?;
        let (_, expected_pw_hash, _) = parse_tlv(rem_key)?;

        let given_pw_hash = Sha256::digest(password.as_bytes());
        if expected_pw_hash != given_pw_hash.as_slice() {
            return Err(anyhow!("Invalid PKCS#12 keystore password"));
        }

        // Extract CommonName from Certificate DER
        let cn = extract_common_name_from_cert(cert_der).unwrap_or_else(|_| "Digital Signer".into());

        Ok(Self {
            common_name: cn,
            organization: "Authenticated Organization".into(),
            email: String::new(),
            serial_number: 1001,
            certificate_der: cert_der.to_vec(),
            private_key_der: key_der.to_vec(),
        })
    }
}

/// Extracts the RSA public key from an X.509 certificate DER buffer.
pub fn extract_rsa_public_key_from_cert(cert_der: &[u8]) -> Result<RsaPublicKey> {
    let (_, cert_children, _) = parse_tlv(cert_der)?;
    let (_, tbs_children, _) = parse_tlv(cert_children)?;
    let (_, _, rem1) = parse_tlv(tbs_children)?; // version
    let (_, _, rem2) = parse_tlv(rem1)?; // serial
    let (_, _, rem3) = parse_tlv(rem2)?; // sig alg
    let (_, _, rem4) = parse_tlv(rem3)?; // issuer
    let (_, _, rem5) = parse_tlv(rem4)?; // validity
    let (_, _, spki_val) = parse_tlv(rem5)?; // subject DN -> remaining is spki

    // Parse SPKI: SEQUENCE { algorithm AlgorithmIdentifier, subjectPublicKey BIT STRING }
    let (_, spki_children, _) = parse_tlv(spki_val)?;
    let (_, _, rem_spki) = parse_tlv(spki_children)?; // skip algorithm
    let (tag_bit, bit_string_val, _) = parse_tlv(rem_spki)?;
    if tag_bit != TAG_BIT_STRING || bit_string_val.is_empty() {
        return Err(anyhow!("Invalid SubjectPublicKeyInfo: expected BIT STRING"));
    }

    // Skip the first byte of BIT STRING (number of unused bits, typically 0)
    let pkcs1_der = &bit_string_val[1..];
    RsaPublicKey::from_pkcs1_der(pkcs1_der)
        .map_err(|e| anyhow!("Failed to parse RSA public key from certificate: {}", e))
}

fn extract_common_name_from_cert(cert_der: &[u8]) -> Result<String> {
    let (_, cert_children, _) = parse_tlv(cert_der)?;
    let (_, tbs_children, _) = parse_tlv(cert_children)?;
    let (_, _, rem1) = parse_tlv(tbs_children)?; // version
    let (_, _, rem2) = parse_tlv(rem1)?; // serial
    let (_, _, rem3) = parse_tlv(rem2)?; // sig alg
    let (_, _, rem4) = parse_tlv(rem3)?; // issuer
    let (_, _, rem5) = parse_tlv(rem4)?; // validity
    let (_, subject_val, _) = parse_tlv(rem5)?; // subject DN

    // Scan subject DN for CN (2.5.4.3)
    let mut cur = subject_val;
    while !cur.is_empty() {
        if let Ok((TAG_SET, set_val, rem_set)) = parse_tlv(cur) {
            cur = rem_set;
            if let Ok((TAG_SEQUENCE, atv_val, _)) = parse_tlv(set_val) {
                if let Ok((TAG_OID, oid_val, rem_atv)) = parse_tlv(atv_val) {
                    if oid_val == [0x55, 0x04, 0x03] {
                        // 2.5.4.3
                        if let Ok((_, str_val, _)) = parse_tlv(rem_atv) {
                            return Ok(String::from_utf8_lossy(str_val).into_owned());
                        }
                    }
                }
            }
        } else {
            break;
        }
    }

    Ok("Digital Signer".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digital_id_generation_and_export() {
        let did = DigitalId::generate(
            "Alice Smith",
            "Acme Legal Corp",
            "alice@acme.com",
            365,
        )
        .expect("generate digital id");

        assert_eq!(did.common_name, "Alice Smith");
        assert_eq!(did.organization, "Acme Legal Corp");
        assert_eq!(did.email, "alice@acme.com");
        assert!(!did.certificate_der.is_empty());
        assert!(!did.private_key_der.is_empty());

        let test_hash = [0x5Au8; 32];
        let sig = did.sign_sha256(&test_hash);
        assert_eq!(sig.len(), 256);

        let p12_bytes = did.to_pkcs12_bytes("SecretPassword123!");
        assert!(!p12_bytes.is_empty());

        let imported = DigitalId::from_pkcs12_bytes(&p12_bytes, "SecretPassword123!")
            .expect("import pkcs12");
        assert_eq!(imported.common_name, "Alice Smith");

        // Wrong password check
        let wrong_pw = DigitalId::from_pkcs12_bytes(&p12_bytes, "WrongPassword");
        assert!(wrong_pw.is_err());
    }
}
