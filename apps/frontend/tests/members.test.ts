import { expect, test } from "bun:test";
import { QueryClient } from "@tanstack/react-query";
import { createBackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { memberList } from "../src/infrastructure/members";
import { isWorkspaceUnavailableToUser } from "../src/infrastructure/workspace";

const organizationId = "b340a05b-bcaf-4bcb-bfe2-e2300ccf32af";
const userId = "019d2fa2-99f8-7d09-87ba-12df307e99ed";
const directoryRow = {
	user_id: userId,
	email: "member@example.org",
	role: "admin",
};

function serve(response: (url: URL) => Response) {
	const client = createBackendClient("https://members.test", () => "");
	client.use({ onRequest: ({ request }) => response(new URL(request.url)) });
	return client;
}

test("changing to a nonmanager role never reuses directory emails or requests the directory", async () => {
	const paths: string[] = [];
	const client = serve((url) => {
		paths.push(url.pathname);
		return Response.json([directoryRow]);
	});
	const cache = new QueryClient({
		defaultOptions: { queries: { staleTime: Infinity } },
	});
	for (const role of ["owner", "admin", "member", "viewer"] as const) {
		const data = await cache.fetchInfiniteQuery(
			memberList(client, organizationId, role),
		);
		const member = { userId, role: "admin" } as const;
		expect(data.pages).toEqual([
			[
				role === "admin" || role === "owner"
					? { ...member, email: directoryRow.email }
					: member,
			],
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
		...memberList(client, organizationId, "owner"),
		pages: 3,
	});
	expect(cursors).toEqual([null, "019d2fa2-99f8-7d09-87ba-000000000049"]);
});

test("a manager's directory must supply an email and surface lost authorization", async () => {
	const cache = new QueryClient();
	const invalid = serve(() =>
		Response.json([{ user_id: userId, role: "member" }]),
	);
	await expect(
		cache.fetchInfiniteQuery(memberList(invalid, organizationId, "admin")),
	).rejects.toThrow(z.ZodError);
	const forbidden = serve(() => new Response(null, { status: 403 }));
	const options = memberList(forbidden, organizationId, "admin");
	await expect(cache.fetchInfiniteQuery(options)).rejects.toThrow();
	expect(
		isWorkspaceUnavailableToUser(cache.getQueryState(options.queryKey)?.error),
	).toBe(true);
});
