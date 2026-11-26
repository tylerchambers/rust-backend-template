import { expect, test } from "bun:test";
import { QueryClient } from "@tanstack/react-query";
import { createBackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { hasMultipleAdmins } from "../src/domain/team";
import {
	changeMemberRole,
	memberList,
	removeMember,
} from "../src/infrastructure/members";
import { isWorkspaceUnavailableToUser } from "../src/infrastructure/workspaceResponse";

const organizationId = "b340a05b-bcaf-4bcb-bfe2-e2300ccf32af";
const userId = "019d2fa2-99f8-7d09-87ba-12df307e99ed";
const directoryRow = {
	user_id: userId,
	email: "member@example.org",
	is_admin: true,
};

function serve(response: (url: URL) => Response) {
	const client = createBackendClient("https://members.test", () => "");
	client.use({ onRequest: ({ request }) => response(new URL(request.url)) });
	return client;
}

test("admin edits require two distinct admins in the loaded roster", () => {
	const admin = { userId, isAdmin: true };
	const firstPage = [admin, { userId: "other", isAdmin: false }];
	expect(hasMultipleAdmins(firstPage)).toBe(false);
	expect(hasMultipleAdmins([...firstPage, admin])).toBe(false);
	expect(
		hasMultipleAdmins([...firstPage, { userId: "next", isAdmin: true }]),
	).toBe(true);
});

test("changing to a nonmanager role never reuses directory emails or requests the directory", async () => {
	const paths: string[] = [];
	const client = serve((url) => {
		paths.push(url.pathname);
		return Response.json([directoryRow]);
	});
	const cache = new QueryClient({
		defaultOptions: { queries: { staleTime: Infinity } },
	});
	for (const isAdmin of [true, false]) {
		const data = await cache.fetchInfiniteQuery(
			memberList(client, organizationId, isAdmin),
		);
		const member = { userId, isAdmin: true };
		expect(data.pages).toEqual([
			[isAdmin ? { ...member, email: directoryRow.email } : member],
		]);
	}
	expect(paths).toEqual([
		`/v1/organizations/${organizationId}/member-directory`,
		`/v1/organizations/${organizationId}/members`,
	]);
});

test("directory pages advance by the last user ID, not an organization ID or an email", async () => {
	const page = Array.from({ length: 50 }, (_, index) => ({
		...directoryRow,
		user_id: `019d2fa2-99f8-7d09-87ba-${String(index).padStart(12, "0")}`,
	}));
	const cursors: (string | null)[] = [];
	const client = serve((url) => {
		const after = url.searchParams.get("after");
		cursors.push(after);
		return Response.json(after ? [] : page);
	});
	await new QueryClient().fetchInfiniteQuery({
		...memberList(client, organizationId, true),
		pages: 3,
	});
	expect(cursors).toEqual([null, "019d2fa2-99f8-7d09-87ba-000000000049"]);
});

test("a manager's directory must supply an email and surface lost authorization", async () => {
	const cache = new QueryClient();
	const invalid = serve(() =>
		Response.json([{ user_id: userId, is_admin: false }]),
	);
	await expect(
		cache.fetchInfiniteQuery(memberList(invalid, organizationId, true)),
	).rejects.toThrow(z.ZodError);
	const forbidden = serve(() => new Response(null, { status: 403 }));
	const options = memberList(forbidden, organizationId, true);
	await expect(cache.fetchInfiniteQuery(options)).rejects.toThrow();
	expect(
		isWorkspaceUnavailableToUser(cache.getQueryState(options.queryKey)?.error),
	).toBe(true);
});

test("change and remove target one organization member with protected POSTs", async () => {
	const origin = "https://members.test";
	const header = { Origin: origin, "x-csrf-token": "csrf-value" };
	const client = createBackendClient(origin, () => header["x-csrf-token"]);
	const writes: { path: string; body: unknown }[] = [];
	client.use({
		onRequest: async ({ request }) => {
			expect(request.method).toBe("POST");
			expect(request.headers.get("Origin")).toBe(origin);
			expect(request.headers.get("x-csrf-token")).toBe(header["x-csrf-token"]);
			expect(request.redirect).toBe("error");
			writes.push({
				path: new URL(request.url).pathname,
				body: request.body === null ? null : await request.json(),
			});
			return new Response(null, { status: 204 });
		},
	});
	await changeMemberRole(client, organizationId, userId, false, header);
	await removeMember(client, organizationId, userId, header);
	const path = `/v1/organizations/${organizationId}/members/${userId}`;
	expect(writes).toEqual([
		{ path: `${path}/role`, body: { is_admin: false } },
		{ path: `${path}/remove`, body: null },
	]);
});
