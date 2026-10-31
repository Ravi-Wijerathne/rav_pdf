use ravpdf_core::*;
use ravpdf_crypto::*;
use sha2::{Digest, Sha256};

#[test]
fn test_signature_preservation_across_incremental_save() {
    let mut doc = PdfDocument::new();
    doc.add_blank_page(612.0, 792.0);

    let did = DigitalId::generate(
        "Adversarial Reviewer",
        "Security Engineering",
        "sec@ravpdf.org",
        365,
    )
    .expect("generate digital id");

    let signed_bytes = sign_pdf_document(
        doc,
        0,
        "EngineerSig",
        &did,
        "Approved for Release",
        "Seattle, WA",
        Some([100.0, 100.0, 300.0, 150.0]),
    )
    .expect("sign document");

    assert!(!signed_bytes.is_empty());
    let orig_len = signed_bytes.len();

    // Verify signature in original document
    let signed_doc = PdfDocument::load_from_bytes(&signed_bytes).expect("load signed doc");
    let sigs = get_document_signatures(&signed_doc, Some(&signed_bytes)).expect("get sigs");
    assert_eq!(sigs.len(), 1);
    assert_eq!(sigs[0].field_name, "EngineerSig");

    // Extract ByteRange array [o1, l1, o2, l2] from the signature dictionary
    let sig_obj = signed_doc.objects.values().find(|obj| {
        if let Some(d) = obj.as_dict() {
            d.get_name("Type") == Some("Sig")
        } else {
            false
        }
    }).expect("find Sig dictionary");

    let sig_dict = sig_obj.as_dict().unwrap();
    let br_arr = sig_dict.get_array("ByteRange").expect("ByteRange array");
    assert_eq!(br_arr.len(), 4);

    let o1 = br_arr[0].as_i64().unwrap() as usize;
    let l1 = br_arr[1].as_i64().unwrap() as usize;
    let o2 = br_arr[2].as_i64().unwrap() as usize;
    let l2 = br_arr[3].as_i64().unwrap() as usize;

    assert_eq!(o1, 0);
    assert_eq!(o2 + l2, orig_len, "Original ByteRange second slice must end at original file length");

    // Compute original digest over the ByteRange
    let mut hasher = Sha256::new();
    hasher.update(&signed_bytes[o1..o1 + l1]);
    hasher.update(&signed_bytes[o2..o2 + l2]);
    let orig_digest: [u8; 32] = hasher.finalize().into();

    // 2. Perform Incremental Save on the signed document
    let mut doc_to_update = PdfDocument::load_from_bytes(&signed_bytes).expect("load for update");
    let new_annot_id = doc_to_update.next_object_id();
    let mut annot = PdfDictionary::new();
    annot.insert("Type", PdfName::new("Annot"));
    annot.insert("Subtype", PdfName::new("Text"));
    annot.insert("Contents", PdfString::from_str("Incremental Comment"));
    doc_to_update.set_object(new_annot_id, Object::Dictionary(annot));

    let updated_bytes = doc_to_update.save_incremental().expect("incremental save on signed doc");

    // 3. Invariance assertions
    assert!(
        updated_bytes.len() > orig_len,
        "Incremental update must append bytes beyond original file length"
    );
    assert_eq!(
        &updated_bytes[0..orig_len],
        &signed_bytes[..],
        "Signed original byte buffer was modified by incremental save"
    );

    // Verify ByteRange slices are bit-for-bit identical
    assert_eq!(
        &updated_bytes[o1..o1 + l1],
        &signed_bytes[o1..o1 + l1],
        "ByteRange first slice modified"
    );
    assert_eq!(
        &updated_bytes[o2..o2 + l2],
        &signed_bytes[o2..o2 + l2],
        "ByteRange second slice modified"
    );

    // Recompute digest over original ByteRange coordinates within the updated buffer
    let mut post_hasher = Sha256::new();
    post_hasher.update(&updated_bytes[o1..o1 + l1]);
    post_hasher.update(&updated_bytes[o2..o2 + l2]);
    let post_digest: [u8; 32] = post_hasher.finalize().into();

    assert_eq!(
        orig_digest, post_digest,
        "SHA-256 cryptographic digest of ByteRange changed after incremental save"
    );
}
