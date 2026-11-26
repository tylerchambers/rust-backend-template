import { infiniteQueryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { workspaceName } from "../domain/workspace";
import { writeHeaders } from "./browser";
import { nextIdCursor, workspacePageSize } from "./workspacePage";
import { workspaceBody } from "./workspaceResponse";

const createdOrganization = z.object({ id: z.uuid() });
const organizationSchema = createdOrganization
	.extend({ name: workspaceName, is_admin: z.boolean() })
	.transform(({ id, name, is_admin }) => ({ id, name, isAdmin: is_admin }));
const organizationsPage = z.array(organizationSchema).max(workspacePageSize);
export type Organization = z.output<typeof organizationSchema>;

export const organizationListKey = ["workspace", "organizations"] as const;

export function organizationList(client: BackendClient) {
	return infiniteQueryOptions({
		queryKey: organizationListKey,
		initialPageParam: "",
		queryFn: async ({ pageParam, signal }) =>
			organizationsPage.parse(
				workspaceBody(
					await client.GET("/v1/organizations", {
						params: { query: pageParam ? { after: pageParam } : {} },
						signal,
					}),
				),
			),
		getNextPageParam: (page) => nextIdCursor(page),
	});
}

export async function createOrganization(
	client: BackendClient,
	name: string,
): Promise<string> {
	const result = await client.POST("/v1/organizations", {
		params: { header: writeHeaders() },
		body: { name },
	});
	return createdOrganization.parse(workspaceBody(result)).id;
}
