#![allow(
    clippy::unwrap_used,
    reason = "Test assertions fail on unexpected rejection"
)]
use domain::{EmailAddress, Error};

#[test]
fn mailbox_policy_normalizes_identity_and_rejects_unsupported_syntax() {
    for input in [
        " Alice+tag@Example.TEST ",
        "a.b@example.test",
        "o'hara@example.test",
    ] {
        let email = EmailAddress::parse(input).unwrap();
        assert_eq!(email.as_str(), input.trim().to_ascii_lowercase());
    }
    for input in [
        "@",
        "a@@b",
        "a@",
        "@example.test",
        "a..b@example.test",
        "a@-example.test",
        "Name <a@example.test>",
        "\"a\"@example.test",
        "a@[127.0.0.1]",
        "é@example.test",
        "a@é.test",
        "a\r\nb@example.test",
        "a b@example.test",
    ] {
        assert_eq!(EmailAddress::parse(input), Err(Error::Invalid), "{input:?}");
    }
}
