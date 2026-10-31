//! Adversarial stress test harness for ISO 32000-1 /ObjStm parsing and Stream Filter decoders.
//! Authored by challenger_m1_1 to stress-test edge cases, malformed streams, table overflows,
//! and adversarial PDF payloads.

use ravpdf_core::filter::{
    decode_ascii85, decode_ascii_hex, decode_dct, decode_dct_with_dimensions, decode_lzw,
    decode_run_length, decode_stream, encode_ascii85, encode_ascii_hex, encode_dct,
    encode_lzw, encode_run_length,
};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfStream};
use ravpdf_core::parser::Parser;
use std::panic::{catch_unwind, AssertUnwindSafe};

// =============================================================================
// Category 1: Object Stream (/ObjStm) Adversarial Tests
// =============================================================================

#[test]
fn test_objstm_missing_or_invalid_n() {
    // Missing /N
    let mut dict = PdfDictionary::new();
    dict.insert("Type", Object::Name(PdfName::new("ObjStm")));
    dict.insert("First", 10i64);
    let stream = PdfStream::with_plain_data(dict.clone(), b"10 0 42".to_vec());
    let res = Parser::parse_object_stream(&stream);
    assert!(res.is_err(), "Missing /N must be rejected");

    // Negative /N
    let mut dict_neg = dict.clone();
    dict_neg.insert("N", -1i64);
    let stream_neg = PdfStream::with_plain_data(dict_neg, b"10 0 42".to_vec());
    let res_neg = Parser::parse_object_stream(&stream_neg);
    assert!(res_neg.is_err(), "Negative /N must be rejected");

    // Non-integer /N (Name)
    let mut dict_name = dict.clone();
    dict_name.insert("N", Object::Name(PdfName::new("NotAnInt")));
    let stream_name = PdfStream::with_plain_data(dict_name, b"10 0 42".to_vec());
    let res_name = Parser::parse_object_stream(&stream_name);
    assert!(res_name.is_err(), "Non-integer /N must be rejected");

    // /N = 0
    let mut dict_zero = dict.clone();
    dict_zero.insert("N", 0i64);
    let stream_zero = PdfStream::with_plain_data(dict_zero, b"".to_vec());
    let res_zero = Parser::parse_object_stream(&stream_zero);
    assert!(res_zero.is_ok(), "/N = 0 should return empty list");
    assert_eq!(res_zero.unwrap().len(), 0);
}

#[test]
fn test_objstm_missing_or_invalid_first() {
    let mut dict = PdfDictionary::new();
    dict.insert("Type", Object::Name(PdfName::new("ObjStm")));
    dict.insert("N", 1i64);

    // Missing /First
    let stream_no_first = PdfStream::with_plain_data(dict.clone(), b"10 0 42".to_vec());
    assert!(
        Parser::parse_object_stream(&stream_no_first).is_err(),
        "Missing /First must be rejected"
    );

    // /First beyond stream length
    let mut dict_beyond = dict.clone();
    dict_beyond.insert("First", 100i64);
    let stream_beyond = PdfStream::with_plain_data(dict_beyond, b"10 0 42".to_vec());
    assert!(
        Parser::parse_object_stream(&stream_beyond).is_err(),
        "/First beyond stream length must be rejected"
    );

    // Negative /First
    let mut dict_neg_first = dict.clone();
    dict_neg_first.insert("First", -5i64);
    let stream_neg_first = PdfStream::with_plain_data(dict_neg_first, b"10 0 42".to_vec());
    let catch_neg = catch_unwind(AssertUnwindSafe(|| {
        Parser::parse_object_stream(&stream_neg_first)
    }));
    assert!(
        catch_neg.is_ok(),
        "Negative /First must NOT panic"
    );
    assert!(
        catch_neg.unwrap().is_err(),
        "Negative /First must return Err"
    );
}

#[test]
fn test_objstm_corrupted_header_tokens() {
    let mut dict = PdfDictionary::new();
    dict.insert("Type", Object::Name(PdfName::new("ObjStm")));
    dict.insert("N", 2i64);

    // Non-integer object number in header: "/bad 0 20 5"
    let data_non_int_obj = b"/bad 0 20 5 (hello) (world)";
    dict.insert("First", 11i64);
    let stream1 = PdfStream::with_plain_data(dict.clone(), data_non_int_obj.to_vec());
    assert!(
        Parser::parse_object_stream(&stream1).is_err(),
        "Non-integer obj_num token in header must be rejected"
    );

    // Negative object number in header: "-10 0 20 5"
    let data_neg_obj = b"-10 0 20 5 (hello) (world)";
    let stream2 = PdfStream::with_plain_data(dict.clone(), data_neg_obj.to_vec());
    assert!(
        Parser::parse_object_stream(&stream2).is_err(),
        "Negative obj_num in header must be rejected"
    );

    // Non-integer relative offset: "10 /offset 20 5"
    let data_non_int_off = b"10 /offset 20 5 (hello) (world)";
    dict.insert("First", 15i64);
    let stream3 = PdfStream::with_plain_data(dict.clone(), data_non_int_off.to_vec());
    assert!(
        Parser::parse_object_stream(&stream3).is_err(),
        "Non-integer offset token in header must be rejected"
    );

    // Negative relative offset in header: "10 -4 20 5"
    let data_neg_off = b"10 -4 20 5 (hello) (world)";
    dict.insert("First", 11i64);
    let stream4 = PdfStream::with_plain_data(dict.clone(), data_neg_off.to_vec());
    assert!(
        Parser::parse_object_stream(&stream4).is_err(),
        "Negative relative offset in header must be rejected"
    );

    // Header truncated mid-pair: specifies N=2, but only 3 tokens provided
    let data_truncated_hdr = b"10 0 20 (hello)";
    dict.insert("First", 7i64);
    let stream5 = PdfStream::with_plain_data(dict.clone(), data_truncated_hdr.to_vec());
    assert!(
        Parser::parse_object_stream(&stream5).is_err(),
        "Truncated header tokens must return Err"
    );
}

#[test]
fn test_objstm_offset_pointing_past_eof() {
    let mut dict = PdfDictionary::new();
    dict.insert("Type", Object::Name(PdfName::new("ObjStm")));
    dict.insert("N", 1i64);
    dict.insert("First", 5i64);

    // Header says object 10 is at relative offset 50, but stream length is only 20
    let data = b"10 50 hello_world_12345";
    let stream = PdfStream::with_plain_data(dict.clone(), data.to_vec());
    let res = Parser::parse_object_stream(&stream);
    assert!(
        res.is_err(),
        "Relative offset pointing past EOF must be cleanly rejected"
    );

    // Arithmetic overflow in relative offset: relative offset near usize::MAX
    let hdr = format!("10 {} ", usize::MAX);
    let mut data_overflow = hdr.clone().into_bytes();
    data_overflow.extend_from_slice(b"42");
    dict.insert("First", hdr.len() as i64);
    let stream_overflow = PdfStream::with_plain_data(dict, data_overflow);
    let res_overflow = catch_unwind(AssertUnwindSafe(|| {
        Parser::parse_object_stream(&stream_overflow)
    }));
    assert!(
        res_overflow.is_ok(),
        "Arithmetic offset overflow must not panic"
    );
    assert!(
        res_overflow.unwrap().is_err(),
        "Offset overflow must return Err"
    );
}

#[test]
fn test_objstm_forbidden_nested_stream() {
    // ISO 32000-1 Clause 7.5.7: "An object stream shall not contain any stream objects."
    let mut dict = PdfDictionary::new();
    dict.insert("Type", Object::Name(PdfName::new("ObjStm")));
    dict.insert("N", 1i64);
    dict.insert("First", 5i64);

    let stream_content = b"10 0 << /Length 5 >> stream\n12345\nendstream";
    let stream = PdfStream::with_plain_data(dict, stream_content.to_vec());
    let res = catch_unwind(AssertUnwindSafe(|| {
        Parser::parse_object_stream(&stream)
    }));
    assert!(res.is_ok(), "Nested stream parsing must not panic");
    assert!(
        res.unwrap().is_err(),
        "Nested stream object must be cleanly rejected per ISO 32000-1 Clause 7.5.7"
    );
}

#[test]
fn test_objstm_corrupted_nested_syntax() {
    let mut dict = PdfDictionary::new();
    dict.insert("Type", Object::Name(PdfName::new("ObjStm")));
    dict.insert("N", 1i64);
    dict.insert("First", 5i64);

    // Corrupted object body: unclosed dictionary
    let stream_bad_body = PdfStream::with_plain_data(dict.clone(), b"10 0 << /Key 123".to_vec());
    assert!(
        Parser::parse_object_stream(&stream_bad_body).is_err(),
        "Malformed object syntax inside /ObjStm must return Err"
    );

    // Unexpected delimiter at start of object
    let stream_bad_delim = PdfStream::with_plain_data(dict, b"10 0 >> >>".to_vec());
    assert!(
        Parser::parse_object_stream(&stream_bad_delim).is_err(),
        "Unexpected delimiter at object offset must return Err"
    );
}

#[test]
fn test_stream_negative_length_panic() {
    let raw = b"1 0 obj\n<< /Length -5 >>\nstream\nhello\nendstream\nendobj\n";
    let parser = Parser::new(raw);
    let res = catch_unwind(AssertUnwindSafe(|| {
        parser.parse_indirect_object_at(0)
    }));
    assert!(
        res.is_ok(),
        "Parser must not panic on negative /Length"
    );
}

#[test]
fn test_xref_stream_negative_w_panic() {
    let raw = b"1 0 obj\n<< /Type /XRef /Size 2 /W [-1 1 0] >>\nstream\n\x01\x10endstream\nendobj\nstartxref\n0\n%%EOF";
    let parser = Parser::new(raw);
    let res = catch_unwind(AssertUnwindSafe(|| {
        parser.parse_xref_and_trailer(0)
    }));
    assert!(
        res.is_ok(),
        "parse_xref_and_trailer must not panic on negative /W values"
    );
}

#[test]
fn test_xref_stream_index_overflow_panic() {
    let raw = b"1 0 obj\n<< /Type /XRef /Size 10 /W [1 1 0] /Index [4294967295 5] >>\nstream\n\x01\x00\x01\x00endstream\nendobj\nstartxref\n0\n%%EOF";
    let parser = Parser::new(raw);
    let res = catch_unwind(AssertUnwindSafe(|| {
        parser.parse_xref_and_trailer(0)
    }));
    assert!(
        res.is_ok(),
        "parse_xref_and_trailer must not panic on overflowing /Index start + count"
    );
}

// =============================================================================
// Category 2: LZW Filter Adversarial & Edge Case Tests
// =============================================================================

#[test]
fn test_lzw_empty_and_minimal_streams() {
    // Empty stream input
    let res_empty = decode_lzw(b"", true);
    assert_eq!(res_empty.unwrap(), Vec::<u8>::new());

    // Stream with only ClearTable (256 in 9 bits: 0b100000000 -> 0x80, 0x00)
    // 256 << 7 = 0x8000
    let clear_only = vec![0x80, 0x00];
    let res_clear = decode_lzw(&clear_only, true);
    assert_eq!(res_clear.unwrap(), Vec::<u8>::new());

    // Stream with only EOD (257 in 9 bits: 0b100000001 -> 0x80, 0x80)
    let eod_only = vec![0x80, 0x80];
    let res_eod = decode_lzw(&eod_only, true);
    assert_eq!(res_eod.unwrap(), Vec::<u8>::new());

    // Stream with ClearTable immediately followed by EOD
    // 256 (9 bits) = 0b100000000, 257 (9 bits) = 0b100000001
    // Total 18 bits: 10000000 01000000 01000000 -> 0x80, 0x40, 0x40
    let clear_then_eod = vec![0x80, 0x40, 0x40];
    let res_ce = decode_lzw(&clear_then_eod, true);
    assert_eq!(res_ce.unwrap(), Vec::<u8>::new());
}

#[test]
fn test_lzw_consecutive_clear_tables_and_trailing_garbage() {
    // Two ClearTable tokens followed by byte 'A' (65) and EOD (257)
    // In 9 bits each:
    // 256: 0b100000000
    // 256: 0b100000000
    // 65:  0b001000001
    // 257: 0b100000001
    // Bit stream: 10000000 01000000 00001000 00110000 00010000 (36 bits -> 5 bytes)
    // 0x80, 0x40, 0x08, 0x30, 0x10
    let payload = vec![0x80, 0x40, 0x08, 0x30, 0x10, 0xFF, 0xAA, 0xBB]; // with trailing garbage
    let res = decode_lzw(&payload, true);
    assert!(res.is_ok(), "Trailing bytes after EOD must be ignored");
    assert_eq!(res.unwrap(), b"A".to_vec());
}

#[test]
fn test_lzw_invalid_initial_kwkwk_code() {
    // Initial code 258 when table length is 258 and prev_entry is None
    // Code 258 in 9 bits: 0b100000010 -> 0x81, 0x00
    let code_258 = vec![0x81, 0x00];
    let res = decode_lzw(&code_258, true);
    assert!(res.is_err(), "Initial code 258 without prev_entry must be rejected");
}

#[test]
fn test_lzw_code_strictly_exceeding_table_size() {
    // Code 500 when table has only 258 entries
    // 500 = 0b111110100 in 9 bits -> 0xFA, 0x80
    let bad_code = vec![0xFA, 0x80];
    let res = decode_lzw(&bad_code, true);
    assert!(res.is_err(), "Code > table.len() must return Err");
}

#[test]
fn test_lzw_early_change_true_large_payload() {
    let mut large_data = Vec::new();
    for i in 0..2000 {
        large_data.extend_from_slice(format!("Token_{:04}_data_chunk_#", i % 100).as_bytes());
    }
    let enc_true = encode_lzw(&large_data, true).expect("encode lzw EarlyChange=true");
    let dec_true = decode_lzw(&enc_true, true).expect("decode lzw EarlyChange=true");
    assert_eq!(
        large_data, dec_true,
        "LZW EarlyChange=true roundtrip must match original bytes for large payload"
    );
}

#[test]
fn test_lzw_early_change_false_large_payload() {
    let mut large_data = Vec::new();
    for i in 0..2000 {
        large_data.extend_from_slice(format!("Token_{:04}_data_chunk_#", i % 100).as_bytes());
    }
    let enc_false = encode_lzw(&large_data, false).expect("encode lzw EarlyChange=false");
    let dec_false = decode_lzw(&enc_false, false).expect("decode lzw EarlyChange=false");
    assert_eq!(
        large_data, dec_false,
        "LZW EarlyChange=false roundtrip must match original bytes for large payload"
    );
}

#[test]
fn test_lzw_maximum_table_overflow_and_clear_table_emission() {
    // Highly diverse sequence that forces the encoder table to exceed 4096 entries
    let mut diverse_data = Vec::new();
    for i in 0..15000 {
        diverse_data.push((i % 256) as u8);
        diverse_data.push(((i * 7 + 13) % 256) as u8);
    }

    let enc = encode_lzw(&diverse_data, true).expect("encode diverse LZW");
    let dec = decode_lzw(&enc, true).expect("decode diverse LZW");
    assert_eq!(
        diverse_data, dec,
        "LZW with > 4096 code entries must cleanly roundtrip through ClearTable reset"
    );
}

#[test]
fn test_lzw_kwkwk_stress_patterns() {
    // Pattern designed to continuously trigger the KwKwK case:
    // Repeated "abababa...", "aaaaaa...", etc.
    let mut data = Vec::new();
    for _ in 0..500 {
        data.extend_from_slice(b"ABCABCABCABCABCABC");
    }

    let enc = encode_lzw(&data, true).expect("encode KwKwK pattern");
    let dec = decode_lzw(&enc, true).expect("decode KwKwK pattern");
    assert_eq!(data, dec, "KwKwK stress pattern must roundtrip perfectly");

    // Single repeating byte 10,000 times
    let single_byte_rep = vec![0x5A; 10000];
    let enc_rep = encode_lzw(&single_byte_rep, true).expect("encode repeated byte");
    let dec_rep = decode_lzw(&enc_rep, true).expect("decode repeated byte");
    assert_eq!(single_byte_rep, dec_rep);
}

// =============================================================================
// Category 3: DCTDecode (JPEG) Adversarial & Edge Case Tests
// =============================================================================

#[test]
fn test_dct_corrupted_and_truncated_headers() {
    // Zero length
    assert!(decode_dct(&[]).is_err(), "Zero-length input to DCTDecode must return Err");

    // Random non-JPEG garbage bytes
    let garbage = b"NotAJpegStreamContentAtAll";
    assert!(decode_dct(garbage).is_err(), "Non-JPEG data must return Err");

    // Truncated JPEG: Only SOI marker 0xFF 0xD8
    let soi_only = [0xFF, 0xD8];
    assert!(decode_dct(&soi_only).is_err(), "Truncated SOI only must return Err");

    // SOI followed by incomplete marker
    let partial_marker = [0xFF, 0xD8, 0xFF, 0xE0, 0x00];
    assert!(decode_dct(&partial_marker).is_err(), "Incomplete marker must return Err");
}

#[test]
fn test_dct_grayscale_vs_rgb() {
    // Generate a valid 8x8 Grayscale JPEG using image crate
    let gray_raw = vec![128u8; 8 * 8];
    let gray_img = image::GrayImage::from_raw(8, 8, gray_raw.clone()).unwrap();
    let mut gray_jpeg = Vec::new();
    {
        let mut cursor = std::io::Cursor::new(&mut gray_jpeg);
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 80);
        enc.encode(gray_img.as_raw(), 8, 8, image::ExtendedColorType::L8).unwrap();
    }

    // Decode grayscale JPEG using decode_dct
    let decoded_rgb = decode_dct(&gray_jpeg);
    assert!(decoded_rgb.is_ok(), "decode_dct must decompress grayscale JPEG without panic");
    // decode_dct converts all decoded images to RGB8
    assert_eq!(
        decoded_rgb.unwrap().len(),
        8 * 8 * 3,
        "Grayscale JPEG decoded via decode_dct should produce 8x8x3 RGB bytes"
    );

    // decode_dct_with_dimensions
    let (dim_bytes, w, h) = decode_dct_with_dimensions(&gray_jpeg).unwrap();
    assert_eq!(w, 8);
    assert_eq!(h, 8);
    assert_eq!(dim_bytes.len(), 8 * 8 * 3);
}

#[test]
fn test_dct_encode_edge_cases() {
    // Buffer size mismatch: 10x10 RGB requires 300 bytes, provided 100 bytes
    let bad_buf = vec![0u8; 100];
    let res = encode_dct(&bad_buf, 10, 10, 80);
    assert!(res.is_err(), "Buffer length mismatch in encode_dct must return Err");

    // Zero dimension width or height
    let res_zero_w = catch_unwind(AssertUnwindSafe(|| {
        encode_dct(&[], 0, 10, 80)
    }));
    assert!(res_zero_w.is_ok(), "encode_dct with width 0 must not panic");
    // JPEG standard does not allow 0 width/height, must either return Err or handle cleanly
    let res_zero = res_zero_w.unwrap();
    assert!(res_zero.is_err() || res_zero.is_ok());

    // Dimension overflow
    let res_overflow = encode_dct(&[0u8; 10], u32::MAX, u32::MAX, 80);
    assert!(res_overflow.is_err(), "Dimension overflow must return Err");
}

// =============================================================================
// Category 4: ASCII85 / ASCIIHex / RunLength Adversarial Tests
// =============================================================================

#[test]
fn test_ascii85_adversarial_cases() {
    // Single character final group: must return Err per ISO 32000-1 Section 7.4.3
    assert!(decode_ascii85(b"9~>").is_err());
    assert!(decode_ascii85(b"x~>").is_err());
    assert!(decode_ascii85(b"!").is_err());

    // Consecutive 'z' abbreviations
    let res_zz = decode_ascii85(b"zzzz~>").expect("decode consecutive z");
    assert_eq!(res_zz, vec![0u8; 16]);

    // 'z' with whitespace in between
    let res_z_ws = decode_ascii85(b"z  z \n\t z~>").expect("decode z with whitespace");
    assert_eq!(res_z_ws, vec![0u8; 12]);

    // 'z' in the middle of a 5-char group: forbidden by ISO 32000-1 Section 7.4.3
    assert!(decode_ascii85(b"!z!!!~>").is_err(), "'z' inside group must be rejected");
    assert!(decode_ascii85(b"!!z!!~>").is_err(), "'z' inside group must be rejected");

    // 5-character tuple overflow: values greater than 2^32 - 1
    // 'uuuuu' represents 85^5 - 1 which exceeds u32::MAX
    let res_overflow = decode_ascii85(b"uuuuu~>");
    assert!(res_overflow.is_err(), "Tuple overflow in ASCII85 must return Err");

    // Characters outside '!'..='u'
    assert!(decode_ascii85(b"v~>").is_err());
    assert!(decode_ascii85(b"w~>").is_err());
    assert!(decode_ascii85(b"{~>").is_err());

    // Empty input and only framing markers
    assert_eq!(decode_ascii85(b"").unwrap(), Vec::<u8>::new());
    assert_eq!(decode_ascii85(b"<~~>").unwrap(), Vec::<u8>::new());
}

#[test]
fn test_ascii_hex_adversarial_cases() {
    // Odd number of nibbles: last nibble padded with 0
    let res_odd_1 = decode_ascii_hex(b"A>").expect("single nibble");
    assert_eq!(res_odd_1, vec![0xA0]);

    let res_odd_3 = decode_ascii_hex(b"ABC>").expect("three nibbles");
    assert_eq!(res_odd_3, vec![0xAB, 0xC0]);

    // Premature termination
    let res_just_term = decode_ascii_hex(b">").expect("just terminator");
    assert_eq!(res_just_term, Vec::<u8>::new());

    // Lowercase and uppercase mixed
    let res_mixed = decode_ascii_hex(b"aBcD>").expect("mixed case");
    assert_eq!(res_mixed, vec![0xAB, 0xCD]);

    // All whitespace characters (spaces, tabs, cr, lf, form feed)
    let res_ws = decode_ascii_hex(b" \t\r\n\x0C 4 \n 1 \t 4 \r 2 > ").expect("whitespace hex");
    assert_eq!(res_ws, b"AB".to_vec());

    // Invalid non-hex characters
    assert!(decode_ascii_hex(b"0123G45>").is_err());
    assert!(decode_ascii_hex(b"123-45>").is_err());
    assert!(decode_ascii_hex(b"123?45>").is_err());

    // Missing terminator (optional in PDF)
    let res_unterminated = decode_ascii_hex(b"414243").expect("unterminated hex");
    assert_eq!(res_unterminated, b"ABC".to_vec());
}

#[test]
fn test_run_length_adversarial_cases() {
    // Truncated copy run: length says 10 bytes follow, only 2 provided
    let truncated_copy = vec![9, 0x01, 0x02];
    assert!(
        decode_run_length(&truncated_copy).is_err(),
        "Truncated copy run must return Err"
    );

    // Truncated repeat run: length says repeat, but stream ends immediately
    let truncated_repeat = vec![200];
    assert!(
        decode_run_length(&truncated_repeat).is_err(),
        "Truncated repeat run must return Err"
    );

    // Only EOD marker (128)
    let res_eod = decode_run_length(&[128]).expect("EOD only");
    assert_eq!(res_eod, Vec::<u8>::new());

    // Bytes following EOD marker: must terminate cleanly and ignore trailing bytes
    let with_trailing = vec![0, 0x41, 128, 0xFF, 0xEE, 0xDD];
    let res_trailing = decode_run_length(&with_trailing).expect("decode with trailing");
    assert_eq!(res_trailing, vec![0x41]);

    // Maximum literal copy run: length = 127 (128 bytes)
    let mut max_copy = vec![127];
    max_copy.extend_from_slice(&(0..128).collect::<Vec<u8>>());
    max_copy.push(128); // EOD
    let res_max_copy = decode_run_length(&max_copy).expect("max copy run");
    assert_eq!(res_max_copy.len(), 128);

    // Maximum repeat run: length = 129 (128 repeated bytes)
    let max_repeat = vec![129, 0x77, 128];
    let res_max_repeat = decode_run_length(&max_repeat).expect("max repeat run");
    assert_eq!(res_max_repeat, vec![0x77; 128]);
}

// =============================================================================
// Category 5: Stream Dispatch & Alias Edge Cases
// =============================================================================

#[test]
fn test_filter_dispatch_aliases_and_errors() {
    // Both full and abbreviated names should work
    let data = b"Testing Stream Filter Dispatcher";

    let flate_enc = encode_ascii_hex(data).unwrap();
    let dec_ahx = decode_stream("AHx", &flate_enc).unwrap();
    let dec_asciihex = decode_stream("/ASCIIHexDecode", &flate_enc).unwrap();
    assert_eq!(dec_ahx, dec_asciihex);

    let a85_enc = encode_ascii85(data).unwrap();
    let dec_a85 = decode_stream("/A85", &a85_enc).unwrap();
    let dec_ascii85 = decode_stream("ASCII85Decode", &a85_enc).unwrap();
    assert_eq!(dec_a85, dec_ascii85);

    let rl_enc = encode_run_length(data).unwrap();
    let dec_rl = decode_stream("RL", &rl_enc).unwrap();
    let dec_runlength = decode_stream("/RunLengthDecode", &rl_enc).unwrap();
    assert_eq!(dec_rl, dec_runlength);

    let lzw_enc = encode_lzw(data, true).unwrap();
    let dec_lzw_short = decode_stream("/LZW", &lzw_enc).unwrap();
    let dec_lzw_long = decode_stream("LZWDecode", &lzw_enc).unwrap();
    assert_eq!(dec_lzw_short, dec_lzw_long);

    // Unsupported filter name returns FilterError
    assert!(decode_stream("UnsupportedFilterXYZ", data).is_err());
    assert!(decode_stream("/Crypt", data).is_err());
}
