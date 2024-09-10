use layer8_interceptor_rs::crypto::{generate_key_pair, KeyUse};

#[test]
fn jwt_to_derivatives_test() {
    let (private_key, public_key) = generate_key_pair(KeyUse::Ecdh).unwrap();
    // able to change to public key and private key derivatives
    _ = private_key.public_key_derivative().unwrap();
    _ = private_key.secret_key_derivative().unwrap();
    _ = public_key.public_key_derivative().unwrap();
    assert!(public_key.secret_key_derivative().is_err());
}
