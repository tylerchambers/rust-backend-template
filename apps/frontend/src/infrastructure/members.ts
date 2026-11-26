import { infiniteQueryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import type { Member } from "../domain/team";
import {
	canManageTeam,
	type WorkspaceRole,
	workspaceRole,
} from "../domain/workspace";
import { writeHeaders } from "./browser";
import { workspacePageSize } from "./workspacePage";
import { workspaceBody } from "./workspaceResponse";

const member = z.object({ user_id: z.uuid(), role: workspaceRole });
const roster = z.array(member).max(workspacePageSize);
const directory = z
	.array(member.extend({ email: z.string().min(1).max(254) }))
	.max(workspacePageSize);

export function memberListKey(organizationId: string) {
	return ["workspace", "members", organizationId] as const;
}

export function memberList(
	client: BackendClient,
	organizationId: string,
	role: WorkspaceRole,
) {
	const manager = canManageTeam(role);
	return infiniteQueryOptions({
		queryKey: [
			...memberListKey(organizationId),
			manager ? "directory" : "roster",
		] as const,
		initialPageParam: "",
		gcTime: 0,
		queryFn: async ({ pageParam, signal }): Promise<Member[]> => {
			const params = {
				path: { organization: organizationId },
				query: pageParam ? { after: pageParam } : {},
			};
			if (manager) {
				const result = await client.GET(
					"/v1/organizations/{organization}/member-directory",
					{ params, signal, cache: "no-store" },
				);
				return directory
					.parse(workspaceBody(result))
					.map(({ user_id, role, email }) => ({
						userId: user_id,
						role,
						email,
					}));
			}
			const result = await client.GET(
				"/v1/organizations/{organization}/members",
				{
					params,
					signal,
					cache: "no-store",
				},
			);
			return roster.parse(workspaceBody(result)).map(({ user_id, role }) => ({
				userId: user_id,
				role,
			}));
		},
		getNextPageParam: (page) =>
			page.length === workspacePageSize ? page.at(-1)?.userId : undefined,
	});
}

export async function changeMemberRole(
	client: BackendClient,
	organizationId: string,
	userId: string,
	role: WorkspaceRole,
	proof: string,
): Promise<void> {
	workspaceBody(
		await client.POST("/v1/organizations/{organization}/members/{user}/role", {
			params: {
				path: { organization: organizationId, user: userId },
				header: writeHeaders(),
			},
			body: { role, proof },
			redirect: "error",
		}),
	);
}

export async function removeMember(
	client: BackendClient,
	organizationId: string,
	userId: string,
	proof: string,
): Promise<void> {
	workspaceBody(
		await client.POST(
			"/v1/organizations/{organization}/members/{user}/remove",
			{
				params: {
					path: { organization: organizationId, user: userId },
					header: writeHeaders(),
				},
				body: { proof },
				redirect: "error",
			},
		),
	);
}
