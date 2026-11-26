import { infiniteQueryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { workspaceName, workspaceRole } from "../domain/workspace";
import { writeHeaders } from "./browser";
import { nextIdCursor, workspacePageSize } from "./workspacePage";
import { workspaceBody } from "./workspaceResponse";

const organizationSchema = z.object({
	id: z.uuid(),
	name: workspaceName,
	role: workspaceRole,
});
const organizationsPage = z.array(organizationSchema).max(workspacePageSize);
const createdOrganization = organizationSchema.pick({ id: true });
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
