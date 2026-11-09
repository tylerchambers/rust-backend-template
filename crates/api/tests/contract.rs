#![allow(clippy::unwrap_used, reason = "Contract assertions fail immediately")]

#[test]
fn every_operation_describes_errors_and_html_pages_describe_text() {
    let contract: serde_json::Value = serde_json::from_str(&api::openapi()).unwrap();
    for (path, item) in contract["paths"].as_object().unwrap() {
        for operation in item.as_object().unwrap().values() {
            assert_eq!(
                operation["responses"]["default"]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ErrorBody",
                "{path}"
            );
            if path.ends_with("confirmation") {
                assert_eq!(
                    operation["responses"]["200"]["content"]["text/html"]["schema"]["type"],
                    "string",
                    "{path}"
                );
            }
        }
    }
    assert_eq!(
        contract["components"]["securitySchemes"]["session"]["name"],
        "__Host-session"
    );
}
