#![allow(clippy::unwrap_used, reason = "Test assertions fail immediately")]

#[test]
fn accepted_mailbox_subset_builds_delivery_recipients() {
    let local_max = format!("{}@example.test", "a".repeat(64));
    let domain_max = format!(
        "a@{}.{}.{}.test",
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(63)
    );
    for input in [
        " Alice+tag@Example.TEST ",
        "a.b@example.test",
        "o'hara@example.test",
        "!#$%&'*+-/=?^_`{|}~@example.test",
        "a@localhost",
        &local_max,
        &domain_max,
    ] {
        let email = domain::EmailAddress::parse(input).unwrap();
        let recipient: lettre::message::Mailbox = email.as_str().parse().unwrap();
        assert_eq!(recipient.email.to_string(), email.as_str());
    }
    for input in [
        format!("{}@example.test", "a".repeat(65)),
        format!("a@{}.test", "b".repeat(64)),
        format!(
            "a@{}.{}.{}.{}.test",
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(63),
            "e".repeat(63)
        ),
    ] {
        assert!(domain::EmailAddress::parse(&input).is_err());
    }
}
