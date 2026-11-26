import { expect, test } from "bun:test";
import { QueryClient } from "@tanstack/react-query";
import {
	type BackendClient,
	createBackendClient,
} from "rust-backend-template-sdk";
import { z } from "zod";
import {
	projectDetail,
	projectList,
	workspaceName,
} from "../src/infrastructure/workspace";

const organizationId = "019d2fa2-99f8-7d09-87ba-12df307e99ed";
const projectId = "b340a05b-bcaf-4bcb-bfe2-e2300ccf32af";
const wireProject = {
	id: projectId,
	organization_id: organizationId,
	name: "Discovery",
	archived: false,
	version: 4,
};

function serve(read: (url: URL) => unknown): BackendClient {
	const client = createBackendClient("https://workspace.test", () => "");
	client.use({
		onRequest: ({ request }) => Response.json(read(new URL(request.url))),
	});
	return client;
}

test("project pages use the final ID as an exclusive cursor only after a full page", async () => {
	const page = Array.from({ length: 50 }, (_, index) => ({
		...wireProject,
		id: `b340a05b-bcaf-4bcb-bfe2-${String(index).padStart(12, "0")}`,
	}));
	const requests: URL[] = [];
	const client = serve((url) => {
		requests.push(url);
		return requests.length === 1 ? page : [wireProject];
	});
	const cache = new QueryClient();
	const data = await cache.fetchInfiniteQuery({
		...projectList(client, organizationId),
		pages: 3,
	});
	expect(requests.map((url) => `${url.pathname}${url.search}`)).toEqual([
		`/v1/organizations/${organizationId}/projects`,
		`/v1/organizations/${organizationId}/projects?after=b340a05b-bcaf-4bcb-bfe2-000000000049`,
	]);
	expect(data.pages[1]).toEqual([
		{
			id: projectId,
			organizationId,
			name: "Discovery",
			archived: false,
			version: 4,
		},
	]);
});

test("project reads never admit a row from a different organization", async () => {
	const cache = new QueryClient();
	const client = serve(() => [
		{ ...wireProject, organization_id: "a1edf503-a64e-4f79-abdd-3769ca5ea4b2" },
	]);
	await expect(
		cache.fetchInfiniteQuery(projectList(client, organizationId)),
	).rejects.toThrow(z.ZodError);
});

test("the authoritative conflict read rejects mismatched IDs and unsafe edit versions", async () => {
	for (const response of [
		{ ...wireProject, id: organizationId },
		{ ...wireProject, version: Number.MAX_SAFE_INTEGER + 1 },
	]) {
		const cache = new QueryClient();
		await expect(
			cache.fetchQuery(
				projectDetail(
					serve(() => response),
					organizationId,
					projectId,
				),
			),
		).rejects.toThrow(z.ZodError);
	}
});

test("names count Unicode code points and reject controls and blank text", () => {
	const character = "\u{10400}";
	expect(workspaceName.safeParse(character.repeat(120)).success).toBe(true);
	for (const name of [character.repeat(121), " \t ", "name\u0080", ""]) {
		expect(workspaceName.safeParse(name).success).toBe(false);
	}
});
