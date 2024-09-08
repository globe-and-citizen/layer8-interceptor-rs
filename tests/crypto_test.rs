use layer8_interceptor_rs::crypto::{generate_key_pair, KeyUse};
use rstest::rstest;

#[rstest]
#[case::case1("hello")]
#[case::case2("world")]
#[case::case3("foo")]
#[case::case4(String::from_utf8_lossy(&(b'0'..=b'z').collect::<Vec<u8>>()).to_string())]
fn encode_decode_test(#[case] input: String) {
    let (private_key, public_key) = generate_key_pair(KeyUse::Ecdh).unwrap();
    let encoded = public_key.symmetric_encrypt(input.as_bytes()).unwrap();
    let decoded = private_key.symmetric_decrypt(&encoded).unwrap();
    assert_eq!(decoded, input.as_bytes());
}
