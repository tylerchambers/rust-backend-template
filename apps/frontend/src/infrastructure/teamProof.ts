import type { BackendClient } from "rust-backend-template-sdk";
import { type WriteHeaders, writeHeaders } from "./browser";
import { workspaceBody } from "./workspaceResponse";

export type TeamProofRequest = {
	organizationId: string;
	action: "invite" | "revoke_invitation" | "change_role" | "remove_member";
	target: string;
};

export async function requestTeamProof(
	client: BackendClient,
	{ organizationId, action, target }: TeamProofRequest,
	header: WriteHeaders = writeHeaders(),
): Promise<void> {
	workspaceBody(
		await client.POST("/v1/auth/reauth/request", {
			params: { header },
			body: { organization_id: organizationId, action, target },
			parseAs: "text",
			redirect: "error",
		}),
	);
}
