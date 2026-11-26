import { expect, test } from "bun:test";
import { QueryClient } from "@tanstack/react-query";
import { createBackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import {
	createInvitation,
	invitationEmail,
	invitationList,
	invitationRoles,
	revokeInvitation,
} from "../src/infrastructure/invitations";

const org = "019d2fa2-99f8-7d09-87ba-12df307e99ed";
const origin = "https://workspace.test";
const header = { Origin: origin, "x-csrf-token": "csrf-value" };
const row = {
	id: "b340a05b-bcaf-4bcb-bfe2-e2300ccf32af",
	email: "person@localhost",
	role: "viewer",
	status: "pending",
	expires_at: "2026-11-27T00:00:00Z",
};

function serve(handler: (request: Request) => Response | Promise<Response>) {
	const client = createBackendClient(origin, () => header["x-csrf-token"]);
	client.use({ onRequest: ({ request }) => handler(request) });
	return client;
}

test("only managers grant invitations and only owners offer ownership", () => {
	expect(invitationRoles("owner")).toEqual([
		"owner",
		"admin",
		"member",
		"viewer",
	]);
	expect(invitationRoles("admin")).toEqual(["admin", "member", "viewer"]);
	expect(invitationRoles("member")).toEqual([]);
	expect(invitationRoles("viewer")).toEqual([]);
});

test("recipient addresses normalize without requiring a public domain suffix", () => {
	expect(invitationEmail.parse(" Alice+tag@LOCALHOST ")).toBe(
		"alice+tag@localhost",
	);
	for (const email of [
		"a..b@localhost",
		"a@-bad.test",
		"é@localhost",
		'"a"@localhost',
	]) {
		expect(invitationEmail.safeParse(email).success).toBe(false);
	}
});

test("history follows the exclusive invitation cursor and validates status and expiry", async () => {
	const page = Array.from({ length: 50 }, (_, i) => ({
		...row,
		id: `b340a05b-bcaf-4bcb-bfe2-${String(i).padStart(12, "0")}`,
	}));
	const paths: string[] = [];
	const client = serve((request) => {
		const url = new URL(request.url);
		paths.push(`${url.pathname}${url.search}`);
		return Response.json(paths.length === 1 ? page : [row]);
	});
	const cache = new QueryClient();
	const history = await cache.fetchInfiniteQuery({
		...invitationList(client, org),
		pages: 3,
	});
	expect(paths).toEqual([
		`/v1/organizations/${org}/invitations`,
		`/v1/organizations/${org}/invitations?after=${page.at(-1)?.id}`,
	]);
	expect(history.pages[1]).toEqual([
		{
			id: row.id,
			email: row.email,
			role: "viewer",
			status: "pending",
			expiresAt: row.expires_at,
		},
	]);
	for (const invalid of [{ status: "unknown" }, { expires_at: "never" }]) {
		const bad = serve(() => Response.json([{ ...row, ...invalid }]));
		await expect(
			new QueryClient().fetchInfiniteQuery(invitationList(bad, org)),
		).rejects.toThrow(z.ZodError);
	}
});

test("create and revoke send the proof only in the protected write body", async () => {
	const requests: { path: string; body: unknown }[] = [];
	const client = serve(async (request) => {
		expect(request.method).toBe("POST");
		expect(request.headers.get("x-csrf-token")).toBe(header["x-csrf-token"]);
		requests.push({
			path: new URL(request.url).pathname,
			body: await request.json(),
		});
		return requests.length === 1
			? Response.json({ id: row.id }, { status: 201 })
			: new Response(null, { status: 204 });
	});
	const draft = { email: row.email, role: "viewer" } as const;
	await createInvitation(client, org, draft, "create-proof", header);
	await revokeInvitation(client, org, row.id, "revoke-proof", header);
	expect(requests).toEqual([
		{
			path: `/v1/organizations/${org}/invitations`,
			body: { ...draft, proof: "create-proof" },
		},
		{
			path: `/v1/organizations/${org}/invitations/${row.id}/revoke`,
			body: { proof: "revoke-proof" },
		},
	]);
});
