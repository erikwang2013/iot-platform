use ecat_access::adapters::tuya::{sign, TuyaAdapter};
use ecat_access::adapter::VendorCreds;

#[test]
fn sign_matches_hmac_sha256_hex() {
    // HMAC-SHA256("client_id" + "1690000000000" + "", "secret") 的 hex
    let s = sign("client_id", "1690000000000", "", "secret");
    assert_eq!(
        s,
        "1efdd7ad5fa94f71f5d5298d37fc18a2049ec0595c8e6cfaf4bb104a027893f2"
    );
}
