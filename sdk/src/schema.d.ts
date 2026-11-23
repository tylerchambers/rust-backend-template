export interface paths {
    "/v1/auth/confirm": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["confirm"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/auth/confirmation": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["confirmation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/auth/logout": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["logout"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/auth/reauth/confirmation": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["proof_confirmation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/auth/reauth/request": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["request_proof"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/auth/request": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["request_login"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/auth/sessions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["sessions"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/auth/sessions/revoke-all": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["revoke_all"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/auth/sessions/rotate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["rotate"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/auth/sessions/{session}/revoke": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["revoke_session"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/invitations/confirmation": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["invitation_confirmation"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/organizations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_organizations"];
        put?: never;
        post: operations["create_org"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/organizations/{organization}/invitations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["invite"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/organizations/{organization}/invitations/{invitation}/accept": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["accept"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/organizations/{organization}/invitations/{invitation}/revoke": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["revoke_invitation"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/organizations/{organization}/members": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_members"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/organizations/{organization}/members/{user}/remove": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["remove_member"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/organizations/{organization}/members/{user}/role": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["change_role"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/organizations/{organization}/projects": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_projects"];
        put?: never;
        post: operations["create_project"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/organizations/{organization}/projects/{project}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["rename_project"];
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        AcceptInvitationRequest: {
            token: string;
        };
        ChangeMemberRoleRequest: {
            proof: string;
            /** @enum {string} */
            role: "owner" | "admin" | "member" | "viewer";
        };
        CreateOrganizationRequest: {
            name: string;
        };
        CreateProjectRequest: {
            name: string;
        };
        ErrorBody: {
            code: string;
        };
        IdentityResponse: {
            /** Format: uuid */
            user_id: string;
        };
        InvitationRequest: {
            email: string;
            proof: string;
            /** @enum {string} */
            role: "owner" | "admin" | "member" | "viewer";
        };
        InvitationResponse: {
            /** Format: uuid */
            id: string;
        };
        LoginConfirmationRequest: {
            token: string;
        };
        LoginRequest: {
            email: string;
        };
        MemberSummaryResponse: {
            /** @enum {string} */
            role: "owner" | "admin" | "member" | "viewer";
            /** Format: uuid */
            user_id: string;
        };
        OrganizationResponse: {
            /** Format: uuid */
            id: string;
        };
        OrganizationSummaryResponse: {
            /** Format: uuid */
            id: string;
            name: string;
            /** @enum {string} */
            role: "owner" | "admin" | "member" | "viewer";
        };
        ProjectResponse: {
            archived: boolean;
            /** Format: uuid */
            id: string;
            name: string;
            /** Format: uuid */
            organization_id: string;
            /** Format: int64 */
            version: number;
        };
        ProofRequest: {
            /** @enum {string} */
            action: "change_role" | "invite" | "revoke_invitation" | "remove_member";
            /** Format: uuid */
            organization_id: string;
            /**
             * Format: uuid
             * @description For `invite`, use `organization_id`; for `revoke_invitation`, use the invitation ID.
             *     For `change_role` and `remove_member`, use the affected user's ID.
             */
            target: string;
        };
        RemoveMemberRequest: {
            proof: string;
        };
        RenameProjectRequest: {
            name: string;
            /** Format: int64 */
            version: number;
        };
        RevokeInvitationRequest: {
            proof: string;
        };
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    confirm: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["LoginConfirmationRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["IdentityResponse"];
                };
            };
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    confirmation: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Scanner-safe page; use SDK parseAs: text */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "text/html": string;
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    logout: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    proof_confirmation: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "text/html": string;
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    request_proof: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ProofRequest"];
            };
        };
        responses: {
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    request_login: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["LoginRequest"];
            };
        };
        responses: {
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            429: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    sessions: {
        parameters: {
            query?: {
                /** @description Exclusive session ID cursor; at most 50 live owned sessions in ascending ID order. Continue with the last ID until an empty page. */
                after?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": string[];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    revoke_all: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    rotate: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    revoke_session: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path: {
                session: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    invitation_confirmation: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "text/html": string;
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_organizations: {
        parameters: {
            query?: {
                /** @description Exclusive organization ID cursor; at most 50 rows */
                after?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["OrganizationSummaryResponse"][];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    create_org: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateOrganizationRequest"];
            };
        };
        responses: {
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["OrganizationResponse"];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    invite: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path: {
                organization: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["InvitationRequest"];
            };
        };
        responses: {
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InvitationResponse"];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    accept: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path: {
                organization: string;
                invitation: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AcceptInvitationRequest"];
            };
        };
        responses: {
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    revoke_invitation: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path: {
                organization: string;
                invitation: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RevokeInvitationRequest"];
            };
        };
        responses: {
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_members: {
        parameters: {
            query?: {
                /** @description Exclusive user ID cursor; at most 50 rows */
                after?: string;
            };
            header?: never;
            path: {
                organization: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MemberSummaryResponse"][];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    remove_member: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path: {
                organization: string;
                user: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RemoveMemberRequest"];
            };
        };
        responses: {
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    change_role: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path: {
                organization: string;
                user: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ChangeMemberRoleRequest"];
            };
        };
        responses: {
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_projects: {
        parameters: {
            query?: {
                /** @description Exclusive project ID cursor; at most 50 rows */
                after?: string;
            };
            header?: never;
            path: {
                organization: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ProjectResponse"][];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    create_project: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path: {
                organization: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateProjectRequest"];
            };
        };
        responses: {
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ProjectResponse"];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    rename_project: {
        parameters: {
            query?: never;
            header: {
                /** @description Exact configured public origin */
                Origin: string;
                /** @description Current csrf cookie value; required with matching csrf cookie */
                "x-csrf-token": string;
            };
            path: {
                organization: string;
                project: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RenameProjectRequest"];
            };
        };
        responses: {
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ProjectResponse"];
                };
            };
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Stable JSON error envelope, including transport, authentication, authorization and availability failures */
            default: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
}
