import { expect, test } from "bun:test";
import { z } from "zod";
import { createBackendClient } from "../src/index";

async function mailboxToken(email: string, subject: string): Promise<string> {
	const mail = process.env.MAILPIT_URL ?? "http://127.0.0.1:58029";
	const listing = z
		.object({
			messages: z.array(
				z.object({
					ID: z.string(),
					Subject: z.string(),
					To: z.array(z.object({ Address: z.string() })),
				}),
			),
		})
		.parse(await (await fetch(`${mail}/api/v1/messages`)).json());
	const message = listing.messages.find(
		(item) =>
			item.Subject === subject && item.To.some((to) => to.Address === email),
	);
	if (!message) throw new Error(`Missing message: ${subject}`);
	const content = z
		.object({ Text: z.string() })
		.parse(await (await fetch(`${mail}/api/v1/message/${message.ID}`)).json());
	const token = /#([a-f0-9]{64})\s*$/m.exec(content.Text)?.[1];
	if (!token) throw new Error(`Missing token: ${subject}`);
	return token;
}

test("generated client reads its account, project, and tenant administration data", async () => {
	const origin = process.env.PUBLIC_ORIGIN ?? "http://localhost:58039";
	const cookies = new Map<string, string>();
	const captureCookies = (response: Response): void => {
		for (const header of response.headers.getSetCookie()) {
			const pair = header.split(";", 1)[0];
			if (!pair) throw new Error("Missing cookie pair");
			const separator = pair.indexOf("=");
			cookies.set(pair.slice(0, separator), pair.slice(separator + 1));
		}
	};
	captureCookies(await fetch(`${origin}/v1/auth/confirmation`));
	const client = createBackendClient(origin, () => cookies.get("csrf") ?? "");
	client.use({
		onRequest({ request }) {
			request.headers.set("Origin", origin);
			request.headers.set(
				"Cookie",
				[...cookies].map(([key, value]) => `${key}=${value}`).join("; "),
			);
			return request;
		},
		onResponse({ response }) {
			captureCookies(response);
			return response;
		},
	});
	const headers = { Origin: origin, "x-csrf-token": cookies.get("csrf") ?? "" };
	const email = `${crypto.randomUUID()}@example.test`;
	const signIn = async () => {
		const requested = await client.POST("/v1/auth/request", {
			params: { header: headers },
			body: { email },
		});
		expect(requested.response.status).toBe(202);
		const login = await client.POST("/v1/auth/confirm", {
			params: { header: headers },
			body: { token: await mailboxToken(email, "Confirm sign in") },
		});
		expect(login.response.status).toBe(200);
		if (!login.data) throw new Error("Login returned no account");
		return { user_id: login.data.user_id, email };
	};
	const account = await signIn();
	const me = await client.GET("/v1/me");
	expect(me.data).toEqual(account);
	expect(me.response.headers.get("cache-control")).toBe("no-store");
	const organization = await client.POST("/v1/organizations", {
		params: { header: headers },
		body: { name: "SDK tenant" },
	});
	if (!organization.data) throw new Error("Organization creation failed");
	const params = {
		header: headers,
		path: { organization: organization.data.id },
	};
	const project = await client.POST(
		"/v1/organizations/{organization}/projects",
		{ params, body: { name: "SDK project" } },
	);
	expect(project.response.status).toBe(201);
	if (!project.data) throw new Error("Project creation returned no project");
	const detail = await client.GET(
		"/v1/organizations/{organization}/projects/{project}",
		{ params: { path: { ...params.path, project: project.data.id } } },
	);
	expect(detail.data).toEqual(project.data);
	const listingResult = await client.GET(
		"/v1/organizations/{organization}/projects",
		{ params: { path: params.path } },
	);
	expect(listingResult.data?.[0]?.name).toBe("SDK project");
	const directory = await client.GET(
		"/v1/organizations/{organization}/member-directory",
		{ params: { path: params.path } },
	);
	expect(directory.data).toEqual([{ ...account, role: "owner" }]);
	const events = await client.GET(
		"/v1/organizations/{organization}/audit-events",
		{ params: { path: params.path } },
	);
	const created = events.data?.find(
		(event) => event.action === "project.created",
	);
	expect(created?.actor_id).toBe(account.user_id);
	expect(created?.subject).toBe(project.data.id);

	const invitee = `${crypto.randomUUID()}@example.test`;
	const org = organization.data.id;
	const proofRequest = await client.POST("/v1/auth/reauth/request", {
		params: { header: headers },
		body: { action: "invite", target: org, organization_id: org },
	});
	expect(proofRequest.response.status).toBe(202);
	const invitation = await client.POST(
		"/v1/organizations/{organization}/invitations",
		{
			params,
			body: {
				email: invitee,
				role: "viewer",
				proof: await mailboxToken(email, "Confirm privileged operation"),
			},
		},
	);
	if (!invitation.data) throw new Error("Invitation creation returned no ID");
	const invitations = await client.GET(
		"/v1/organizations/{organization}/invitations",
		{ params: { path: params.path } },
	);
	const pending = invitations.data?.[0];
	expect(pending?.id).toBe(invitation.data.id);
	expect(pending?.email).toBe(invitee);
	expect(pending?.status).toBe("pending");
	const rest = await client.GET(
		"/v1/organizations/{organization}/invitations",
		{ params: { path: params.path, query: { after: invitation.data.id } } },
	);
	expect(rest.data).toEqual([]);
	expect(
		(await client.POST("/v1/auth/logout", { params: { header: headers } }))
			.response.status,
	).toBe(204);
	expect(await signIn()).toEqual(account);
	const sessions = await client.GET("/v1/auth/sessions");
	expect(sessions.data).toHaveLength(1);
	const session = sessions.data?.[0];
	if (!session) throw new Error("Returning client has no active session");
	for (const [id, status] of [
		[crypto.randomUUID(), 404],
		[session, 204],
	] as const) {
		const revoked = await client.POST("/v1/auth/sessions/{session}/revoke", {
			params: { header: headers, path: { session: id } },
		});
		expect(revoked.response.status).toBe(status);
	}
	expect((await client.GET("/v1/me")).error?.code).toBe("unauthenticated");
}, 15000);
