import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { workspaceName } from "../domain/workspace";
import { writeHeaders } from "./browser";
import { nextIdCursor, workspacePageSize } from "./workspacePage";
import { workspaceBody } from "./workspaceResponse";

const projectSchema = z
	.object({
		id: z.uuid(),
		organization_id: z.uuid(),
		name: workspaceName,
		version: z.int().positive(),
		archived: z.boolean(),
	})
	.transform(({ organization_id, ...project }) => ({
		...project,
		organizationId: organization_id,
	}));
export type Project = z.output<typeof projectSchema>;

function scopedProject(organizationId: string, projectId?: string) {
	return projectSchema.refine(
		(project) =>
			project.organizationId === organizationId &&
			(projectId === undefined || project.id === projectId),
	);
}

export function projectList(client: BackendClient, organizationId: string) {
	const pageSchema = z
		.array(scopedProject(organizationId))
		.max(workspacePageSize);
	return infiniteQueryOptions({
		queryKey: ["workspace", "projects", organizationId] as const,
		initialPageParam: "",
		queryFn: async ({ pageParam, signal }) =>
			pageSchema.parse(
				workspaceBody(
					await client.GET("/v1/organizations/{organization}/projects", {
						params: {
							path: { organization: organizationId },
							query: pageParam ? { after: pageParam } : {},
						},
						signal,
					}),
				),
			),
		getNextPageParam: (page) => nextIdCursor(page),
	});
}

export function projectDetail(
	client: BackendClient,
	organizationId: string,
	projectId: string,
) {
	return queryOptions({
		queryKey: ["workspace", "project", organizationId, projectId] as const,
		queryFn: async ({ signal }) =>
			scopedProject(organizationId, projectId).parse(
				workspaceBody(
					await client.GET(
						"/v1/organizations/{organization}/projects/{project}",
						{
							params: {
								path: { organization: organizationId, project: projectId },
							},
							signal,
						},
					),
				),
			),
	});
}

export async function createProject(
	client: BackendClient,
	organizationId: string,
	name: string,
): Promise<Project> {
	const result = await client.POST(
		"/v1/organizations/{organization}/projects",
		{
			params: {
				path: { organization: organizationId },
				header: writeHeaders(),
			},
			body: { name },
		},
	);
	return scopedProject(organizationId).parse(workspaceBody(result));
}

export async function renameProject(
	client: BackendClient,
	project: Pick<Project, "organizationId" | "id" | "version">,
	name: string,
): Promise<Project> {
	const result = await client.PATCH(
		"/v1/organizations/{organization}/projects/{project}",
		{
			params: {
				path: { organization: project.organizationId, project: project.id },
				header: writeHeaders(),
			},
			body: { name, version: project.version },
		},
	);
	return scopedProject(project.organizationId, project.id).parse(
		workspaceBody(result),
	);
}
