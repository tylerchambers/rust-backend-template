#![allow(clippy::unwrap_used, reason = "Contract assertions fail immediately")]

#[test]
fn resource_operations_keep_their_routes_and_operation_ids() {
    let contract: serde_json::Value = serde_json::from_str(&api::openapi()).unwrap();
    let expected = [
        ("post", "/v1/auth/request", "request_login"),
        ("post", "/v1/auth/confirm", "confirm"),
        ("get", "/v1/auth/confirmation", "confirmation"),
        ("post", "/v1/auth/logout", "logout"),
        ("post", "/v1/auth/reauth/request", "request_proof"),
        ("get", "/v1/auth/reauth/confirmation", "proof_confirmation"),
        ("get", "/v1/auth/sessions", "sessions"),
        (
            "post",
            "/v1/auth/sessions/{session}/revoke",
            "revoke_session",
        ),
        ("post", "/v1/auth/sessions/revoke-all", "revoke_all"),
        ("post", "/v1/auth/sessions/rotate", "rotate"),
        (
            "get",
            "/v1/invitations/confirmation",
            "invitation_confirmation",
        ),
        ("get", "/v1/organizations", "list_organizations"),
        ("post", "/v1/organizations", "create_org"),
        (
            "get",
            "/v1/organizations/{organization}/members",
            "list_members",
        ),
        (
            "post",
            "/v1/organizations/{organization}/members/{user}/remove",
            "remove_member",
        ),
        (
            "post",
            "/v1/organizations/{organization}/members/{user}/role",
            "change_role",
        ),
        (
            "post",
            "/v1/organizations/{organization}/invitations",
            "invite",
        ),
        (
            "post",
            "/v1/organizations/{organization}/invitations/{invitation}/accept",
            "accept",
        ),
        (
            "post",
            "/v1/organizations/{organization}/invitations/{invitation}/revoke",
            "revoke_invitation",
        ),
        (
            "get",
            "/v1/organizations/{organization}/projects",
            "list_projects",
        ),
        (
            "post",
            "/v1/organizations/{organization}/projects",
            "create_project",
        ),
        (
            "patch",
            "/v1/organizations/{organization}/projects/{project}",
            "rename_project",
        ),
    ];
    let actual_count: usize = contract["paths"]
        .as_object()
        .unwrap()
        .values()
        .map(|item| item.as_object().unwrap().len())
        .sum();
    assert_eq!(actual_count, expected.len());
    for (method, path, operation_id) in expected {
        assert_eq!(
            contract["paths"][path][method]["operationId"], operation_id,
            "{method} {path}"
        );
    }
}

#[test]
fn semantically_distinct_commands_have_resource_owned_schemas() {
    let contract: serde_json::Value = serde_json::from_str(&api::openapi()).unwrap();
    for (path, schema) in [
        ("/v1/auth/confirm", "LoginConfirmationRequest"),
        ("/v1/organizations", "CreateOrganizationRequest"),
        (
            "/v1/organizations/{organization}/projects",
            "CreateProjectRequest",
        ),
        (
            "/v1/organizations/{organization}/invitations/{invitation}/accept",
            "AcceptInvitationRequest",
        ),
        (
            "/v1/organizations/{organization}/invitations/{invitation}/revoke",
            "RevokeInvitationRequest",
        ),
        (
            "/v1/organizations/{organization}/members/{user}/remove",
            "RemoveMemberRequest",
        ),
    ] {
        assert_eq!(
            contract["paths"][path]["post"]["requestBody"]["content"]["application/json"]["schema"]
                ["$ref"],
            format!("#/components/schemas/{schema}")
        );
        assert_eq!(
            contract["components"]["schemas"][schema]["additionalProperties"], false,
            "{schema}"
        );
    }
    assert_eq!(
        contract["paths"]["/v1/organizations/{organization}/invitations"]["post"]["responses"]["201"]
            ["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/InvitationResponse"
    );
}

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
