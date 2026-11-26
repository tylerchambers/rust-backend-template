import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { writeHeaders } from "./browser";

const pageSize = 50;
export const workspaceName = z
	.string()
	.refine(
		(value) =>
			value.trim().length > 0 &&
			[...value].length <= 120 &&
			!/\p{Cc}/u.test(value),
		"Use 1–120 characters, including a non-space character, with no control characters.",
	);
const organizationSchema = z.object({
	id: z.uuid(),
	name: workspaceName,
	role: z.enum(["owner", "admin", "member", "viewer"]),
});
const organizationsPage = z.array(organizationSchema).max(pageSize);
const createdOrganization = organizationSchema.pick({ id: true });
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

class WorkspaceHttpError extends Error {
	constructor(readonly status: number) {
		super(`Workspace request failed (${status})`);
	}
}

function body(result: { response: Response; data?: unknown }): unknown {
	if (!result.response.ok) throw new WorkspaceHttpError(result.response.status);
	return result.data;
}

export function isVersionConflict(error: unknown): boolean {
	return error instanceof WorkspaceHttpError && error.status === 409;
}

export function isWorkspaceUnavailableToUser(error: unknown): boolean {
	return (
		error instanceof WorkspaceHttpError &&
		(error.status === 401 || error.status === 403 || error.status === 404)
	);
}

export function workspaceError(error: unknown): string {
	if (error instanceof WorkspaceHttpError) {
		switch (error.status) {
			case 400:
				return "The server could not accept this request. Check the name and try again.";
			case 401:
				return "Your session has expired. Sign in again to continue.";
			case 403:
				return "You don’t have permission to do that in this organization.";
			case 404:
				return "This item no longer exists or is not available to you.";
			case 429:
				return "Too many requests. Wait a moment before trying again.";
		}
	}
	if (error instanceof z.ZodError)
		return "The server returned unexpected workspace data.";
	return "The request could not be completed. Check your connection and try again.";
}

function nextCursor(page: { id: string }[]): string | undefined {
	return page.length === pageSize ? page.at(-1)?.id : undefined;
}

export function organizationList(client: BackendClient) {
	return infiniteQueryOptions({
		queryKey: ["workspace", "organizations"] as const,
		initialPageParam: "",
		queryFn: async ({ pageParam, signal }) =>
			organizationsPage.parse(
				body(
					await client.GET("/v1/organizations", {
						params: { query: pageParam ? { after: pageParam } : {} },
						signal,
					}),
				),
			),
		getNextPageParam: (page) => nextCursor(page),
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
	return createdOrganization.parse(body(result)).id;
}

export function projectList(client: BackendClient, organizationId: string) {
	const pageSchema = z.array(scopedProject(organizationId)).max(pageSize);
	return infiniteQueryOptions({
		queryKey: ["workspace", "projects", organizationId] as const,
		initialPageParam: "",
		queryFn: async ({ pageParam, signal }) =>
			pageSchema.parse(
				body(
					await client.GET("/v1/organizations/{organization}/projects", {
						params: {
							path: { organization: organizationId },
							query: pageParam ? { after: pageParam } : {},
						},
						signal,
					}),
				),
			),
		getNextPageParam: (page) => nextCursor(page),
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
				body(
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
	return scopedProject(organizationId).parse(body(result));
}

export async function renameProject(
	client: BackendClient,
	project: Project,
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
	return scopedProject(project.organizationId, project.id).parse(body(result));
}
