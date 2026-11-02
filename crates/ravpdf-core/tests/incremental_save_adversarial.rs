use ravpdf_core::*;
use std::collections::HashSet;

/// Helper: creates a basic single-page PDF document buffer
fn create_test_base_pdf() -> (Vec<u8>, ObjectId, ObjectId, ObjectId) {
    let mut doc = PdfDocument::new();
    let root_id = doc.root_id;
    let pages_id = doc.pages_root_id().unwrap();
    let page_id = doc.add_blank_page(612.0, 792.0);
    let bytes = doc.to_bytes().expect("serialize base pdf");
    (bytes, root_id, pages_id, page_id)
}

// =========================================================================
// 1. BYTE INVARIANCE TESTS
// =========================================================================

#[test]
fn test_byte_invariance_standard_newline() {
    let (mut base_bytes, _, _, _) = create_test_base_pdf();
    if !base_bytes.ends_with(b"\n") {
        base_bytes.push(b'\n');
    }
    let orig_len = base_bytes.len();

    let mut doc = PdfDocument::load_from_bytes(&base_bytes).expect("load base");
    let new_obj_id = doc.next_object_id();
    doc.set_object(new_obj_id, Object::Integer(42));

    let updated_bytes = doc.save_incremental().expect("save incremental");

    assert!(updated_bytes.len() > orig_len);
    assert_eq!(
        &updated_bytes[0..orig_len],
        &base_bytes[..],
        "Byte-by-byte invariance violated on document ending with newline"
    );
}

#[test]
fn test_byte_invariance_crlf_ending() {
    let (base_bytes, _, _, _) = create_test_base_pdf();
    // Normalize to CRLF ending
    let mut crlf_bytes = base_bytes.clone();
    while crlf_bytes.ends_with(b"\n") || crlf_bytes.ends_with(b"\r") {
        crlf_bytes.pop();
    }
    crlf_bytes.extend_from_slice(b"\r\n");
    let orig_len = crlf_bytes.len();

    let mut doc = PdfDocument::load_from_bytes(&crlf_bytes).expect("load base crlf");
    let new_obj_id = doc.next_object_id();
    doc.set_object(new_obj_id, Object::Integer(99));

    let updated_bytes = doc.save_incremental().expect("save incremental");

    assert!(updated_bytes.len() > orig_len);
    assert_eq!(
        &updated_bytes[0..orig_len],
        &crlf_bytes[..],
        "Byte-by-byte invariance violated on document ending with CRLF"
    );
}

#[test]
fn test_byte_invariance_no_trailing_newline() {
    let (base_bytes, _, _, _) = create_test_base_pdf();
    // Strip trailing newlines so it ends strictly with "%%EOF"
    let mut stripped = base_bytes.clone();
    while stripped.ends_with(b"\n") || stripped.ends_with(b"\r") {
        stripped.pop();
    }
    assert!(stripped.ends_with(b"%%EOF"));
    let orig_len = stripped.len();

    let mut doc = PdfDocument::load_from_bytes(&stripped).expect("load stripped base");
    let new_obj_id = doc.next_object_id();
    doc.set_object(
        new_obj_id,
        Object::String(PdfString::from_str("InvarianceTest")),
    );

    let updated_bytes = doc.save_incremental().expect("save incremental");

    assert!(updated_bytes.len() > orig_len);
    assert_eq!(
        &updated_bytes[0..orig_len],
        &stripped[..],
        "Byte-by-byte invariance violated on document without trailing newline"
    );
}

#[test]
fn test_byte_invariance_cr_only_ending() {
    let (base_bytes, _, _, _) = create_test_base_pdf();
    let mut cr_only = base_bytes.clone();
    while cr_only.ends_with(b"\n") || cr_only.ends_with(b"\r") {
        cr_only.pop();
    }
    cr_only.push(b'\r');
    let orig_len = cr_only.len();

    let mut doc = PdfDocument::load_from_bytes(&cr_only).expect("load cr base");
    let new_obj_id = doc.next_object_id();
    doc.set_object(new_obj_id, Object::Boolean(true));

    let updated_bytes = doc.save_incremental().expect("save incremental");

    assert!(updated_bytes.len() > orig_len);
    assert_eq!(
        &updated_bytes[0..orig_len],
        &cr_only[..],
        "Byte-by-byte invariance violated on document ending with CR only"
    );
}

// =========================================================================
// 2. MULTI-GENERATION CHAINING (10 SEQUENTIAL REVISIONS)
// =========================================================================

#[test]
fn test_multi_generation_chaining_10_revisions() {
    let (base_bytes, root_id, _pages_id, page_id) = create_test_base_pdf();
    let mut current_doc = PdfDocument::load_from_bytes(&base_bytes).expect("load gen 0");

    let mut gen_bytes_history: Vec<Vec<u8>> = vec![base_bytes.clone()];
    let mut startxref_history: Vec<usize> = Vec::new();

    let initial_parser = Parser::new(&base_bytes);
    startxref_history.push(
        initial_parser
            .find_startxref()
            .expect("find startxref gen 0"),
    );

    // Add custom trailer key in Gen 0
    current_doc.trailer.insert(
        "DocumentTitle",
        PdfString::from_str("OriginalDocumentTitle"),
    );

    // Perform 10 sequential incremental updates
    for gen in 1..=10 {
        let prev_len = gen_bytes_history.last().unwrap().len();

        // 1. Add a newly created object in each generation
        let new_id = current_doc.next_object_id();
        let mut new_dict = PdfDictionary::new();
        new_dict.insert("Generation", gen as i64);
        new_dict.insert("Tag", PdfString::from_str(&format!("ObjectGen{}", gen)));
        current_doc.set_object(new_id, Object::Dictionary(new_dict));

        // 2. Modify an existing object across generations (e.g. modify catalog or pages)
        if gen % 2 == 0 {
            // Modify root catalog with generation stamp
            if let Some(cat) = current_doc.catalog_mut() {
                cat.insert("LastModifiedGen", gen as i64);
                current_doc.mark_dirty(root_id);
            }
        } else {
            // Modify a custom object previously added
            let test_mod_id = ObjectId::new(100, 0);
            current_doc.set_object(test_mod_id, Object::Integer(gen as i64 * 100));
        }

        // Save incremental update
        let updated_bytes = current_doc
            .save_incremental()
            .expect("save incremental gen");

        // Verify byte invariance for this generation against previous generation
        assert!(updated_bytes.len() > prev_len);
        assert_eq!(
            &updated_bytes[0..prev_len],
            &gen_bytes_history.last().unwrap()[..],
            "Byte invariance violated at generation {}",
            gen
        );

        // Record history
        let parser = Parser::new(&updated_bytes);
        let startxref = parser.find_startxref().expect("find startxref");
        assert!(
            startxref >= prev_len,
            "XRef offset must be in the newly appended slice"
        );
        startxref_history.push(startxref);
        gen_bytes_history.push(updated_bytes.clone());

        // Parse this generation independently from bytes
        let reloaded = PdfDocument::load_from_bytes(&updated_bytes).expect("reload doc gen");

        // Verify root and pages integrity
        assert_eq!(reloaded.root_id, root_id);
        assert_eq!(reloaded.page_count(), 1);
        assert!(reloaded.get_object(page_id).is_some());

        // Verify latest values resolved
        assert!(
            reloaded.get_object(new_id).is_some(),
            "Object added in gen {} must resolve",
            gen
        );
        if gen % 2 == 0 {
            let cat = reloaded.catalog().expect("catalog");
            assert_eq!(
                cat.get_i64("LastModifiedGen"),
                Some(gen as i64),
                "Catalog should reflect modification in gen {}",
                gen
            );
        } else {
            let mod_obj = reloaded
                .get_object(ObjectId::new(100, 0))
                .expect("mod obj 100");
            assert_eq!(mod_obj.as_i64(), Some(gen as i64 * 100));
        }

        // Verify trailer inheritance: original DocumentTitle must be inherited across all generations
        assert_eq!(
            reloaded.trailer.get_str("DocumentTitle"),
            Some("OriginalDocumentTitle".to_string()),
            "Trailer dictionary lost parent metadata at generation {}",
            gen
        );

        current_doc = reloaded;
    }

    // Now test the backwards /Prev chain from Generation 10 all the way to Generation 0
    let final_bytes = gen_bytes_history.last().unwrap();
    let parser = Parser::new(final_bytes);
    let mut current_startxref = parser.find_startxref().expect("final startxref");

    let mut hops = 0;
    let mut visited_offsets = HashSet::new();

    while current_startxref > 0 {
        assert!(
            visited_offsets.insert(current_startxref),
            "Cyclic loop detected during /Prev traversal at offset {}",
            current_startxref
        );

        let (_, trailer) = parser
            .parse_xref_and_trailer(current_startxref)
            .expect("parse xref at revision hop");

        if let Some(prev) = trailer.get_i64("Prev") {
            assert!(
                (prev as usize) < current_startxref,
                "/Prev pointer ({}) must strictly point backward before current xref ({})",
                prev,
                current_startxref
            );
            current_startxref = prev as usize;
            hops += 1;
        } else {
            // Reached base generation
            assert_eq!(
                current_startxref, startxref_history[0],
                "Earliest /Prev must resolve to Generation 0 startxref"
            );
            break;
        }
    }

    assert_eq!(
        hops, 10,
        "Expected exactly 10 hops backwards through /Prev chain"
    );
}

#[test]
fn test_cyclic_prev_does_not_lock_up() {
    // Construct synthetic PDF with an exact circular /Prev reference:
    // xref A points to xref B, and xref B points to xref A.
    let (base_bytes, _, _, _) = create_test_base_pdf();

    let mut cyclic_pdf = base_bytes.clone();
    let xref_a_pos = cyclic_pdf.len();

    // Formatted with fixed-width Prev offset for exact calculation:
    // "xref\n0 1\n0000000000 65535 f \ntrailer\n<< /Size 10 /Prev 1234567890 /Root 1 0 R >>\n" = 80 bytes
    let template =
        "xref\n0 1\n0000000000 65535 f \ntrailer\n<< /Size 10 /Prev 0000000000 /Root 1 0 R >>\n";
    let expected_b_pos = xref_a_pos + template.len();

    let xref_a_str = format!(
        "xref\n0 1\n0000000000 65535 f \ntrailer\n<< /Size 10 /Prev {:010} /Root 1 0 R >>\n",
        expected_b_pos
    );
    cyclic_pdf.extend_from_slice(xref_a_str.as_bytes());

    let actual_xref_b_pos = cyclic_pdf.len();
    assert_eq!(actual_xref_b_pos, expected_b_pos);

    let xref_b_str = format!(
        "xref\n0 1\n0000000000 65535 f \ntrailer\n<< /Size 10 /Prev {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        xref_a_pos, actual_xref_b_pos
    );
    cyclic_pdf.extend_from_slice(xref_b_str.as_bytes());

    // When parsing with startxref at xref_b, it follows Prev to xref_a, which points back to xref_b.
    let cyclic_parser = Parser::new(&cyclic_pdf);
    let start = std::time::Instant::now();
    let res = cyclic_parser.parse_xref_and_trailer(actual_xref_b_pos);
    let elapsed = start.elapsed();

    // Assert cycle was broken without locking up or blowing the stack
    assert!(
        elapsed.as_millis() < 500,
        "Cyclic /Prev took too long ({:?}), likely locked up in infinite loop",
        elapsed
    );
    // The parser successfully breaks the cycle and returns
    assert!(
        res.is_ok(),
        "Parser gracefully recovers without cyclic lockup"
    );
}

// =========================================================================
// 3. DELETION TOMBSTONING TESTS
// =========================================================================

#[test]
fn test_deletion_tombstoning_gen0_matches() {
    // When free entry specifies exact matching generation (gen 0)
    let (base_bytes, _root_id, _pages_id, _) = create_test_base_pdf();
    let mut doc0 = PdfDocument::load_from_bytes(&base_bytes).expect("load base");

    let custom_id = doc0.next_object_id();
    doc0.set_object(custom_id, Object::String(PdfString::from_str("SecretData")));
    let rev0_bytes = doc0.to_bytes().expect("serialize rev0");

    let parser = Parser::new(&rev0_bytes);
    let rev0_startxref = parser.find_startxref().expect("rev0 startxref");

    // Incremental update freeing custom_id with gen 0
    let mut inc_bytes = rev0_bytes.clone();
    let inc_xref_pos = inc_bytes.len();
    let inc_str = format!(
        "xref\n{} 1\n0000000000 {:05} f \ntrailer\n<< /Size 10 /Prev {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        custom_id.number, custom_id.generation, rev0_startxref, inc_xref_pos
    );
    inc_bytes.extend_from_slice(inc_str.as_bytes());

    let doc1 = PdfDocument::load_from_bytes(&inc_bytes).expect("load rev1");
    assert!(
        doc1.get_object(custom_id).is_none(),
        "Deleted object with matching gen must not be resurrected"
    );
}

#[test]
fn test_deletion_tombstoning_gen1_iso32000_fails() {
    // ISO 32000-1 Clause 7.5.4 requires free entries to set generation = gen + 1 (or 65535)
    let (base_bytes, _root_id, _pages_id, _) = create_test_base_pdf();
    let mut doc0 = PdfDocument::load_from_bytes(&base_bytes).expect("load base");

    let custom_id = doc0.next_object_id();
    doc0.set_object(
        custom_id,
        Object::String(PdfString::from_str("ConfidentialNote")),
    );
    let rev0_bytes = doc0.to_bytes().expect("serialize rev0");

    let parser = Parser::new(&rev0_bytes);
    let rev0_startxref = parser.find_startxref().expect("rev0 startxref");

    // Incremental update freeing custom_id with gen + 1 (gen 1)
    let mut inc_bytes = rev0_bytes.clone();
    let inc_xref_pos = inc_bytes.len();
    let inc_str = format!(
        "xref\n{} 1\n0000000000 00001 f \ntrailer\n<< /Size 10 /Prev {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        custom_id.number, rev0_startxref, inc_xref_pos
    );
    inc_bytes.extend_from_slice(inc_str.as_bytes());

    let doc1 = PdfDocument::load_from_bytes(&inc_bytes).expect("load rev1");

    // In current implementation, doc1 incorrectly resurrects custom_id!
    // We document whether custom_id is resurrected:
    let resurrected = doc1.get_object(custom_id).is_some();
    if resurrected {
        eprintln!(
            "VULNERABILITY CONFIRMED: Object {} was resurrected because free entry used generation 1!",
            custom_id
        );
    }
    assert!(
        doc1.get_object(custom_id).is_none(),
        "Deleted object marked free with gen 1 per ISO 32000-1 clause 7.5.4 MUST NOT be resurrected"
    );
}

#[test]
fn test_deletion_tombstoning_gen65535_fails() {
    // Standard PDF deletion marker when object cannot be reused: gen = 65535
    let (base_bytes, _, _, _) = create_test_base_pdf();
    let mut doc0 = PdfDocument::load_from_bytes(&base_bytes).expect("load base");

    let custom_id = doc0.next_object_id();
    doc0.set_object(
        custom_id,
        Object::String(PdfString::from_str("RedactedSSN")),
    );
    let rev0_bytes = doc0.to_bytes().expect("serialize rev0");

    let parser = Parser::new(&rev0_bytes);
    let rev0_startxref = parser.find_startxref().expect("rev0 startxref");

    // Incremental update freeing custom_id with 65535
    let mut inc_bytes = rev0_bytes.clone();
    let inc_xref_pos = inc_bytes.len();
    let inc_str = format!(
        "xref\n{} 1\n0000000000 65535 f \ntrailer\n<< /Size 10 /Prev {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        custom_id.number, rev0_startxref, inc_xref_pos
    );
    inc_bytes.extend_from_slice(inc_str.as_bytes());

    let doc1 = PdfDocument::load_from_bytes(&inc_bytes).expect("load rev1");
    assert!(
        doc1.get_object(custom_id).is_none(),
        "Deleted object marked free with gen 65535 MUST NOT be resurrected"
    );
}

#[test]
fn test_deletion_tombstoning_xref_stream_gen1_fails() {
    // In an ISO 32000-1 XRef stream, Type 0 entry:
    // Field 1: 0 (Type 0)
    // Field 2: next free object number
    // Field 3: generation number (e.g. 1 per Clause 7.5.8)
    let (base_bytes, _, _, _) = create_test_base_pdf();
    let mut doc0 = PdfDocument::load_from_bytes(&base_bytes).expect("load base");

    let custom_id = doc0.next_object_id();
    doc0.set_object(
        custom_id,
        Object::String(PdfString::from_str("ConfidentialContract")),
    );
    let rev0_bytes = doc0.to_bytes().expect("serialize rev0");

    let parser = Parser::new(&rev0_bytes);
    let rev0_startxref = parser.find_startxref().expect("rev0 startxref");

    // Build incremental XRef stream that marks custom_id as free with gen 1
    let mut inc_bytes = rev0_bytes.clone();
    let xref_stm_id = ObjectId::new(custom_id.number + 1, 0);
    let xref_stm_offset = inc_bytes.len();

    // W = [1, 1, 2] -> entry size = 4
    // Entry: Type 0, next_free = 0, gen = 1 -> [0x00, 0x00, 0x00, 0x01]
    let entry_bytes = vec![0x00, 0x00, 0x00, 0x01];
    let xref_stm_header = format!(
        "{} {} obj\n<< /Type /XRef /Size {} /Index [{} 1] /W [1 1 2] /Prev {} /Root 1 0 R /Length {} >>\nstream\n",
        xref_stm_id.number, xref_stm_id.generation,
        xref_stm_id.number + 1,
        custom_id.number,
        rev0_startxref,
        entry_bytes.len()
    );
    inc_bytes.extend_from_slice(xref_stm_header.as_bytes());
    inc_bytes.extend_from_slice(&entry_bytes);
    inc_bytes.extend_from_slice(b"\nendstream\nendobj\n");

    let footer = format!("startxref\n{}\n%%EOF\n", xref_stm_offset);
    inc_bytes.extend_from_slice(footer.as_bytes());

    let doc1 = PdfDocument::load_from_bytes(&inc_bytes).expect("load rev1 xref stm");
    assert!(
        doc1.get_object(custom_id).is_none(),
        "Deleted object in XRef Stream marked free with gen 1 MUST NOT be resurrected"
    );
}

#[test]
fn test_deletion_tombstoning_multi_revision_chaining() {
    // Gen 0: Base doc with Obj 4 (gen 0)
    let (base_bytes, _root_id, _pages_id, _) = create_test_base_pdf();
    let mut doc0 = PdfDocument::load_from_bytes(&base_bytes).expect("load base");

    let custom_id = doc0.next_object_id();
    doc0.set_object(
        custom_id,
        Object::String(PdfString::from_str("DeletedInRev1")),
    );
    let rev0_bytes = doc0.to_bytes().expect("serialize rev0");

    let parser0 = Parser::new(&rev0_bytes);
    let rev0_startxref = parser0.find_startxref().expect("rev0 startxref");

    // Gen 1: Free Obj 4 with matching gen 0
    let mut rev1_bytes = rev0_bytes.clone();
    let rev1_xref_pos = rev1_bytes.len();
    let rev1_str = format!(
        "xref\n{} 1\n0000000000 00000 f \ntrailer\n<< /Size 10 /Prev {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        custom_id.number, rev0_startxref, rev1_xref_pos
    );
    rev1_bytes.extend_from_slice(rev1_str.as_bytes());

    let doc1 = PdfDocument::load_from_bytes(&rev1_bytes).expect("load rev1");
    assert!(
        doc1.get_object(custom_id).is_none(),
        "Must be deleted in rev1"
    );

    // Gen 2: Add Obj 10 (Obj 4 is NOT mentioned in Gen 2 xref table!)
    let mut doc1_mut = doc1;
    let obj10_id = ObjectId::new(10, 0);
    doc1_mut.set_object(obj10_id, Object::Integer(5555));
    let rev2_bytes = doc1_mut.save_incremental().expect("save rev2");

    // Load Gen 2: Does Obj 4 stay deleted, or does Gen 2 resurrect Obj 4 from Gen 0?
    let doc2 = PdfDocument::load_from_bytes(&rev2_bytes).expect("load rev2");
    assert!(
        doc2.get_object(obj10_id).is_some(),
        "Obj 10 must exist in rev2"
    );
    assert!(
        doc2.get_object(custom_id).is_none(),
        "Obj 4 was deleted in Rev 1; Rev 2 MUST NOT resurrect Obj 4 from Rev 0"
    );
}

// =========================================================================
// 4. SIGNATURE PRESERVATION & BYTERANGE TESTS
// =========================================================================

#[test]
fn test_synthetic_signature_byterange_preservation() {
    // 1. Build a synthetic document containing a signature with ByteRange
    let (mut base_bytes, _, _, _) = create_test_base_pdf();
    // Normalize to clean newline
    if !base_bytes.ends_with(b"\n") {
        base_bytes.push(b'\n');
    }
    let orig_len = base_bytes.len();

    // Define a ByteRange inside base_bytes:
    // Say len1 = orig_len / 2, offset2 = len1 + 64, len2 = orig_len - offset2
    let len1 = orig_len / 2;
    let offset2 = len1 + 64;
    let len2 = orig_len - offset2;

    // Simulate signature digest over orig_bytes[0..len1] and orig_bytes[offset2..offset2+len2]
    let slice1_orig = base_bytes[0..len1].to_vec();
    let slice2_orig = base_bytes[offset2..offset2 + len2].to_vec();

    // Simple robust hash over slices: FNV-1a 64-bit and 32-bit checksums
    fn compute_digest(s1: &[u8], s2: &[u8]) -> u128 {
        let mut hash: u128 = 0x0cbf_29ce_4842_2232_5100_0000_01b3;
        for &b in s1.iter().chain(s2.iter()) {
            hash ^= b as u128;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    let orig_digest = compute_digest(&slice1_orig, &slice2_orig);

    // 2. Perform an incremental update
    let mut doc = PdfDocument::load_from_bytes(&base_bytes).expect("load base");
    let annot_id = doc.next_object_id();
    doc.set_object(
        annot_id,
        Object::String(PdfString::from_str("AnnotationAfterSignature")),
    );

    let updated_bytes = doc.save_incremental().expect("save incremental");

    // 3. Assert exact byte slice invariance of the original ByteRange
    assert!(updated_bytes.len() > orig_len);
    assert_eq!(
        &updated_bytes[0..orig_len],
        &base_bytes[..],
        "Incremental update modified the original byte buffer"
    );

    let slice1_updated = &updated_bytes[0..len1];
    let slice2_updated = &updated_bytes[offset2..offset2 + len2];

    assert_eq!(
        slice1_updated,
        &slice1_orig[..],
        "ByteRange slice 1 was modified during incremental save"
    );
    assert_eq!(
        slice2_updated,
        &slice2_orig[..],
        "ByteRange slice 2 was modified during incremental save"
    );

    let updated_digest = compute_digest(slice1_updated, slice2_updated);
    assert_eq!(
        updated_digest, orig_digest,
        "Cryptographic ByteRange digest altered after incremental save"
    );
}
