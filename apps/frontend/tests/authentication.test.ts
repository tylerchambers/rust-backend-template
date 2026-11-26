import { expect, test } from "bun:test";
import { createBackendClient } from "rust-backend-template-sdk";
import {
	Authentication,
	AuthenticationError,
	authenticationMessage,
} from "../src/infrastructure/authentication";

const origin = "https://workspace.example";
const cookie = "a".repeat(64);
const signal = new AbortController().signal;
const account = {
	user_id: "0199cf64-3983-790b-8f65-1c61039eb75c",
	email: "person@localhost",
};

function adapter(
	serve: (request: Request) => Response | Promise<Response>,
	csrf: () => string = () => cookie,
): Authentication {
	const client = createBackendClient(origin, csrf);
	client.use({ onRequest: ({ request }) => serve(request) });
	return new Authentication(client, () => ({
		Origin: origin,
		"x-csrf-token": csrf(),
	}));
}

test("only deliberate authentication errors are displayed to users", () => {
	expect(authenticationMessage(new AuthenticationError("Sign in again."))).toBe(
		"Sign in again.",
	);
	expect(authenticationMessage(new Error("private detail"))).toBe(
		"The service could not be reached. Check your connection and try again.",
	);
	expect(authenticationMessage("private detail", "Session unavailable.")).toBe(
		"Session unavailable.",
	);
});

test("an authenticated account comes from the current session, not a saved identity", async () => {
	const auth = adapter((request) => {
		expect(request.url).toBe(`${origin}/v1/me`);
		expect(request.cache).toBe("no-store");
		return Response.json(account);
	});
	expect(await auth.currentAccount(signal)).toEqual({
		id: account.user_id,
		email: account.email,
	});
});

test("only an unauthorized identity response means signed out", async () => {
	const denied = adapter(() =>
		Response.json({ code: "unauthenticated" }, { status: 401 }),
	);
	expect(await denied.currentAccount(signal)).toBeNull();
	for (const status of [403, 503]) {
		const unavailable = adapter(() =>
			Response.json({ code: "failure" }, { status }),
		);
		await expect(unavailable.currentAccount(signal)).rejects.toBeInstanceOf(
			AuthenticationError,
		);
	}
	const malformed = adapter(() => Response.json({ email: account.email }));
	await expect(malformed.currentAccount(signal)).rejects.toBeInstanceOf(
		AuthenticationError,
	);
});

test("a new browser obtains the CSRF cookie from the inert page before requesting a link", async () => {
	let token = "";
	const paths: string[] = [];
	const auth = adapter(
		async (request) => {
			const path = new URL(request.url).pathname;
			paths.push(`${request.method} ${path}`);
			if (path === "/v1/auth/confirmation") {
				token = cookie;
				return new Response("<!doctype html><title>Confirm sign in</title>", {
					headers: { "Content-Type": "text/html" },
				});
			}
			expect(request.headers.get("x-csrf-token")).toBe(cookie);
			expect(await request.json()).toEqual({ email: account.email });
			return new Response(null, { status: 202 });
		},
		() => token,
	);
	await auth.requestLink(`  ${account.email}  `);
	expect(paths).toEqual(["GET /v1/auth/confirmation", "POST /v1/auth/request"]);
});

test("a missing CSRF cookie after bootstrap never initiates a login request", async () => {
	const methods: string[] = [];
	const auth = adapter(
		(request) => {
			methods.push(request.method);
			return new Response("<!doctype html>");
		},
		() => "",
	);
	await expect(auth.requestLink(account.email)).rejects.toBeInstanceOf(
		AuthenticationError,
	);
	expect(methods).toEqual(["GET"]);
});

test("a restored session also bootstraps a missing CSRF cookie before mounting writable pages", async () => {
	let token = "";
	const paths: string[] = [];
	const auth = adapter(
		(request) => {
			const path = new URL(request.url).pathname;
			paths.push(path);
			if (path === "/v1/me") return Response.json(account);
			token = cookie;
			return new Response("<!doctype html>");
		},
		() => token,
	);
	expect(await auth.currentAccount(signal)).toEqual({
		id: account.user_id,
		email: account.email,
	});
	expect(paths).toEqual(["/v1/me", "/v1/auth/confirmation"]);
});

test("cancelling identity lookup during CSRF preparation aborts the confirmation request", async () => {
	const controller = new AbortController();
	const auth = adapter(
		(request) => {
			if (request.url === `${origin}/v1/me`) return Response.json(account);
			expect(request.url).toBe(`${origin}/v1/auth/confirmation`);
			controller.abort();
			request.signal.throwIfAborted();
			return new Response(null);
		},
		() => "",
	);
	await expect(auth.currentAccount(controller.signal)).rejects.toMatchObject({
		name: "AbortError",
	});
});

test("rate limits do not look like a successful link request", async () => {
	const auth = adapter(() =>
		Response.json({ code: "rate_limited" }, { status: 429 }),
	);
	await expect(auth.requestLink(account.email)).rejects.toThrow(
		"Too many requests",
	);
});

test("sign-out uses the backend; a server failure is not a confirmed sign-out", async () => {
	const auth = adapter((request) => {
		expect(request.url).toBe(`${origin}/v1/auth/logout`);
		expect(request.method).toBe("POST");
		expect(request.headers.get("x-csrf-token")).toBe(cookie);
		return new Response(null, { status: 204 });
	});
	await auth.signOut();
	const unavailable = adapter(() =>
		Response.json({ code: "unavailable" }, { status: 503 }),
	);
	await expect(unavailable.signOut()).rejects.toBeInstanceOf(
		AuthenticationError,
	);
});
