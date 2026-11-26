import { expect, test } from "bun:test";
import { QueryClient } from "@tanstack/react-query";
import { createBackendClient } from "rust-backend-template-sdk";
import { AuthenticationError } from "../src/infrastructure/authentication";
import { changeSession, sessionList } from "../src/infrastructure/sessions";

const origin = "https://workspace.example";
const headers = { Origin: origin, "x-csrf-token": "a".repeat(64) };
const first = "0199cf64-3983-790b-8f65-1c61039eb75c";
const last = "0199cf64-3983-790b-8f65-1c61039eb75d";

function serve(response: (request: Request) => Response) {
	const client = createBackendClient(origin, () => headers["x-csrf-token"]);
	client.use({ onRequest: ({ request }) => response(request) });
	return client;
}

test("session pages continue from the final ID until an empty page", async () => {
	const paths: string[] = [];
	const client = serve((request) => {
		const url = new URL(request.url);
		paths.push(url.pathname + url.search);
		expect(request.cache).toBe("no-store");
		return Response.json(paths.length === 1 ? [first, last] : []);
	});
	const result = await new QueryClient().fetchInfiniteQuery({
		...sessionList(client),
		pages: 3,
	});
	expect(paths).toEqual([
		"/v1/auth/sessions",
		`/v1/auth/sessions?after=${last}`,
	]);
	expect(result.pages).toEqual([[first, last], []]);
});

test("session reads reject malformed IDs and do not treat an error as an empty list", async () => {
	for (const response of [
		Response.json(["not-a-session"]),
		Response.json([], { status: 401 }),
	]) {
		await expect(
			new QueryClient().fetchInfiniteQuery(sessionList(serve(() => response))),
		).rejects.toBeInstanceOf(AuthenticationError);
	}
});

test("session changes send authenticated CSRF-protected POSTs to their exact targets", async () => {
	const paths: string[] = [];
	const client = serve((request) => {
		paths.push(new URL(request.url).pathname);
		expect(request.method).toBe("POST");
		expect(request.credentials).toBe("include");
		expect(request.headers.get("Origin")).toBe(origin);
		expect(request.headers.get("x-csrf-token")).toBe(headers["x-csrf-token"]);
		expect(request.redirect).toBe("error");
		return new Response(null, { status: 204 });
	});
	await changeSession(client, { kind: "rotate" }, headers);
	await changeSession(client, { kind: "revoke", id: first }, headers);
	await changeSession(client, { kind: "revokeAll" }, headers);
	expect(paths).toEqual([
		"/v1/auth/sessions/rotate",
		`/v1/auth/sessions/${first}/revoke`,
		"/v1/auth/sessions/revoke-all",
	]);
});

test("revocation requires a confirmed 204 and keeps the response body out of errors", async () => {
	for (const [status, message] of [
		[200, "The service did not confirm the session request."],
		[401, "Your session has expired. Sign in again."],
		[403, "The security check failed. Check your session and try again."],
		[404, "This session is no longer available."],
		[503, "The service did not confirm the session request."],
	] as const) {
		const client = serve(() => new Response("private detail", { status }));
		const result = changeSession(
			client,
			{ kind: "revoke", id: first },
			headers,
		);
		await expect(result).rejects.toThrow(message);
	}
});
