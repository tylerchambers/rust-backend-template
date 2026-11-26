import { infiniteQueryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { type WriteHeaders, writeHeaders } from "./browser";
import { type WorkspaceRole, workspaceBody, workspaceRole } from "./workspace";

const pageSize = 50;
export const invitationEmail = z
	.string()
	.trim()
	.max(254)
	.pipe(z.email({ pattern: z.regexes.html5Email }))
	.refine((email) => {
		const local = email.slice(0, email.indexOf("@"));
		return (
			local.length <= 64 &&
			!local.startsWith(".") &&
			!local.endsWith(".") &&
			!local.includes("..")
		);
	}, "Use an unquoted email address without empty parts.")
	.transform((email) => email.toLowerCase());
const invitationId = z.object({ id: z.uuid() });
const invitationsPage = z
	.array(
		invitationId
			.extend({
				email: z.string().min(1).max(254),
				role: workspaceRole,
				status: z.enum(["pending", "accepted", "revoked", "expired"]),
				expires_at: z.iso.datetime({ offset: true }),
			})
			.transform(({ expires_at, ...row }) => ({
				...row,
				expiresAt: expires_at,
			})),
	)
	.max(pageSize);

export function invitationRoles(role: WorkspaceRole): WorkspaceRole[] {
	switch (role) {
		case "owner":
			return workspaceRole.options;
		case "admin":
			return ["admin", "member", "viewer"];
		case "member":
		case "viewer":
			return [];
	}
}

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
		getNextPageParam: (page) =>
			page.length === pageSize ? page.at(-1)?.id : undefined,
	});
}

export type InvitationDraft = { email: string; role: WorkspaceRole };

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
