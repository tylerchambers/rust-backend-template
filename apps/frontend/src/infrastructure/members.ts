import { infiniteQueryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import type { Member } from "../domain/team";
import { type WriteHeaders, writeHeaders } from "./browser";
import { workspacePageSize } from "./workspacePage";
import { workspaceBody } from "./workspaceResponse";

const member = z.object({ user_id: z.uuid(), is_admin: z.boolean() });
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
	isAdmin: boolean,
) {
	return infiniteQueryOptions({
		queryKey: [
			...memberListKey(organizationId),
			isAdmin ? "directory" : "roster",
		] as const,
		initialPageParam: "",
		gcTime: 0,
		queryFn: async ({ pageParam, signal }): Promise<Member[]> => {
			const params = {
				path: { organization: organizationId },
				query: pageParam ? { after: pageParam } : {},
			};
			if (isAdmin) {
				const result = await client.GET(
					"/v1/organizations/{organization}/member-directory",
					{ params, signal, cache: "no-store" },
				);
				return directory
					.parse(workspaceBody(result))
					.map(({ user_id, is_admin, email }) => ({
						userId: user_id,
						isAdmin: is_admin,
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
			return roster
				.parse(workspaceBody(result))
				.map(({ user_id, is_admin }) => ({
					userId: user_id,
					isAdmin: is_admin,
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
	isAdmin: boolean,
	header: WriteHeaders = writeHeaders(),
): Promise<void> {
	workspaceBody(
		await client.POST("/v1/organizations/{organization}/members/{user}/role", {
			params: {
				path: { organization: organizationId, user: userId },
				header,
			},
			body: { is_admin: isAdmin },
			redirect: "error",
		}),
	);
}

export async function removeMember(
	client: BackendClient,
	organizationId: string,
	userId: string,
	header: WriteHeaders = writeHeaders(),
): Promise<void> {
	workspaceBody(
		await client.POST(
			"/v1/organizations/{organization}/members/{user}/remove",
			{
				params: {
					path: { organization: organizationId, user: userId },
					header,
				},
				redirect: "error",
			},
		),
	);
}
