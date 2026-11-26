import { expect, test } from "bun:test";
import { QueryClient } from "@tanstack/react-query";
import { createBackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import {
	canRevokeInvitation,
	grantableRoles,
	reviewInvitation,
} from "../src/domain/team";
import {
	createInvitation,
	invitationList,
	revokeInvitation,
} from "../src/infrastructure/invitations";
import { requestTeamProof } from "../src/infrastructure/teamProof";

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
	expect(grantableRoles("owner")).toEqual([
		"owner",
		"admin",
		"member",
		"viewer",
	]);
	expect(grantableRoles("admin")).toEqual(["admin", "member", "viewer"]);
	expect(grantableRoles("member")).toEqual([]);
	expect(grantableRoles("viewer")).toEqual([]);
	expect(reviewInvitation(row.email, "owner", "admin")).toEqual({
		status: "invalid-role",
	});
	expect(reviewInvitation(row.email, "viewer", "member")).toEqual({
		status: "invalid-role",
	});
});

test("only pending invitations can be revoked", () => {
	expect(canRevokeInvitation("pending")).toBe(true);
	for (const status of ["accepted", "expired", "revoked"] as const) {
		expect(canRevokeInvitation(status)).toBe(false);
	}
});

test("recipient addresses normalize without requiring a public domain suffix", () => {
	expect(reviewInvitation(" Alice+tag@LOCALHOST ", "owner", "owner")).toEqual({
		status: "ready",
		draft: { email: "alice+tag@localhost", role: "owner" },
	});
	for (const email of [
		"a..b@localhost",
		"a@-bad.test",
		"é@localhost",
		'"a"@localhost',
	]) {
		expect(reviewInvitation(email, "viewer", "admin")).toEqual({
			status: "invalid-email",
		});
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

test("requesting a proof binds the organization, action and target in a protected POST", async () => {
	const client = serve(async (request) => {
		expect(request.url).toBe(`${origin}/v1/auth/reauth/request`);
		expect(request.method).toBe("POST");
		expect(request.redirect).toBe("error");
		expect(request.headers.get("Origin")).toBe(origin);
		expect(request.headers.get("x-csrf-token")).toBe(header["x-csrf-token"]);
		expect(await request.json()).toEqual({
			organization_id: org,
			action: "revoke_invitation",
			target: row.id,
		});
		return new Response(null, { status: 202 });
	});
	await requestTeamProof(
		client,
		{ organizationId: org, action: "revoke_invitation", target: row.id },
		header,
	);
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
