import { infiniteQueryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { type InvitationDraft, invitationStatus } from "../domain/team";
import { workspaceRole } from "../domain/workspace";
import { type WriteHeaders, writeHeaders } from "./browser";
import { nextIdCursor, workspacePageSize } from "./workspacePage";
import { workspaceBody } from "./workspaceResponse";

const invitationId = z.object({ id: z.uuid() });
const invitationsPage = z
	.array(
		invitationId
			.extend({
				email: z.string().min(1).max(254),
				role: workspaceRole,
				status: invitationStatus,
				expires_at: z.iso.datetime({ offset: true }),
			})
			.transform(({ expires_at, ...row }) => ({
				...row,
				expiresAt: expires_at,
			})),
	)
	.max(workspacePageSize);

export function invitationList(client: BackendClient, organizationId: string) {
	return infiniteQueryOptions({
		queryKey: ["workspace", "invitations", organizationId] as const,
		initialPageParam: "",
		gcTime: 0,
		queryFn: async ({ pageParam, signal }) =>
			invitationsPage.parse(
				workspaceBody(
					await client.GET("/v1/organizations/{organization}/invitations", {
						params: {
							path: { organization: organizationId },
							query: pageParam ? { after: pageParam } : {},
						},
						signal,
						cache: "no-store",
					}),
				),
			),
		getNextPageParam: (page) => nextIdCursor(page),
	});
}

export async function createInvitation(
	client: BackendClient,
	organizationId: string,
	draft: InvitationDraft,
	proof: string,
	header: WriteHeaders = writeHeaders(),
): Promise<void> {
	const result = await client.POST(
		"/v1/organizations/{organization}/invitations",
		{
			params: { path: { organization: organizationId }, header },
			body: { ...draft, proof },
			redirect: "error",
		},
	);
	invitationId.parse(workspaceBody(result));
}

export async function revokeInvitation(
	client: BackendClient,
	organizationId: string,
	id: string,
	proof: string,
	header: WriteHeaders = writeHeaders(),
): Promise<void> {
	workspaceBody(
		await client.POST(
			"/v1/organizations/{organization}/invitations/{invitation}/revoke",
			{
				params: {
					path: { organization: organizationId, invitation: id },
					header,
				},
				body: { proof },
				redirect: "error",
			},
		),
	);
}
