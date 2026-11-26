import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { createRequire } from "node:module";
import { checkTenantAccess } from "./tenant.mjs";

// The SDK owns the pinned browser-test dependency; this is an application journey.
const require = createRequire(
	new URL("../../sdk/package.json", import.meta.url),
);
const { chromium } = require("playwright");
const origin = process.env.PUBLIC_ORIGIN ?? "http://localhost:58039";
const mail = process.env.MAILPIT_URL ?? "http://127.0.0.1:58029";
const readMessages = new Set();

async function mailText(email, subject) {
	const deadline = Date.now() + 15000;
	while (Date.now() < deadline) {
		const response = await fetch(`${mail}/api/v1/messages`);
		assert.equal(response.status, 200);
		const { messages } = await response.json();
		const message = messages.find(
			(item) =>
				!readMessages.has(item.ID) &&
				item.Subject === subject &&
				item.To.some((to) => to.Address === email),
		);
		if (message) {
			readMessages.add(message.ID);
			const content = await fetch(`${mail}/api/v1/message/${message.ID}`);
			assert.equal(content.status, 200);
			return (await content.json()).Text;
		}
		await Bun.sleep(100);
	}
	throw new Error(`Missing captured email: ${subject}`);
}

const browser = await chromium.launch({ headless: true });
try {
	const context = await browser.newContext({ baseURL: origin });
	const page = await context.newPage();
	const openReplacement = async (tab, fragment) => {
		await tab.evaluate((value) => {
			location.hash = value;
		}, fragment);
		await tab.waitForFunction(() => location.hash === "");
	};
	await page.goto("/");
	const email = `${randomUUID()}@example.test`;
	await page.getByLabel("Email address").fill(email);
	const requested = page.waitForResponse((response) =>
		response.url().endsWith("/v1/auth/request"),
	);
	await page.getByRole("button", { name: "Send sign-in link" }).click();
	assert.equal((await requested).status(), 202);
	await page.getByRole("heading", { name: "Check your inbox" }).waitFor();
	const token = /confirmation#([a-f0-9]{64})/.exec(
		await mailText(email, "Confirm sign in"),
	)?.[1];
	assert.ok(token);
	await page.goto(`/v1/auth/confirmation#${token}`);
	assert.equal((await page.request.get("/v1/auth/sessions")).status(), 401);
	const otherTab = await page.context().newPage();
	await otherTab.goto("/v1/auth/confirmation");
	const confirmationResponse = page.waitForResponse((response) =>
		response.url().endsWith("/v1/auth/confirm"),
	);
	await page
		.getByRole("button", { name: "Confirm sign in", exact: true })
		.click();
	const response = await confirmationResponse;
	assert.equal(response.status(), 200);
	await page.getByText("Signed in", { exact: true }).waitFor();
	await openReplacement(page, "another-login-link");
	assert.equal(await page.locator("#result").textContent(), "");
	assert.equal((await page.request.get("/v1/auth/sessions")).status(), 200);
	const cookies = await page.context().cookies();
	const session = cookies.find((cookie) => cookie.name === "__Host-session");
	assert.ok(session?.httpOnly && session.secure && session.sameSite === "Lax");
	const post = async (path, body, includeCsrf = true) =>
		page.evaluate(
			async ({ path, body, includeCsrf }) => {
				const csrf = document.cookie
					.split("; ")
					.find((part) => part.startsWith("csrf="))
					?.slice(5);
				const headers = { "content-type": "application/json" };
				if (includeCsrf) headers["x-csrf-token"] = csrf ?? "";
				const response = await fetch(path, {
					method: "POST",
					headers,
					body: JSON.stringify(body),
				});
				return { status: response.status, text: await response.text() };
			},
			{ path, body, includeCsrf },
		);
	await page.getByRole("link", { name: "Open Project desk" }).click();
	await page.getByLabel("Organization name").fill("Browser proof tenant");
	const organization = page.waitForResponse(
		(response) =>
			response.request().method() === "POST" &&
			response.url().endsWith("/v1/organizations"),
	);
	await page.getByRole("button", { name: "Create organization" }).click();
	const created = await organization;
	assert.equal(created.status(), 201);
	const { id: org } = await created.json();
	await page
		.getByRole("link", { name: "Browser proof tenant", exact: true })
		.click();
	await page.getByLabel("Project name").fill("First project");
	await page.getByRole("button", { name: "Create project" }).click();
	await page.getByRole("button", { name: "Rename First project" }).click();
	await otherTab.goto(page.url());
	await otherTab.getByRole("button", { name: "Rename First project" }).click();
	await otherTab
		.getByLabel("New project name")
		.fill("Another editor's project");
	await otherTab.getByRole("button", { name: "Save name" }).click();
	await otherTab
		.getByRole("heading", { name: "Another editor's project" })
		.waitFor();
	await page.getByLabel("New project name").fill("Renamed project");
	await page.getByRole("button", { name: "Save name" }).click();
	await page.getByText("This project changed while you were editing").waitFor();
	assert.equal(
		await page.getByLabel("New project name").inputValue(),
		"Renamed project",
	);
	await page
		.getByRole("button", { name: "Continue with this version" })
		.click();
	await page.getByRole("button", { name: "Save name" }).click();
	await page.getByRole("heading", { name: "Renamed project" }).waitFor();
	await page.reload();
	await page.getByRole("heading", { name: "Renamed project" }).waitFor();
	const invitePath = `/v1/organizations/${org}/invitations`;
	const invitee = `${randomUUID()}@example.test`;
	assert.equal(
		(await post(invitePath, { email: invitee, role: "viewer", proof: "" }))
			.status,
		403,
	);
	assert.equal(
		(
			await post("/v1/auth/reauth/request", {
				action: "invite",
				target: org,
				organization_id: org,
			})
		).status,
		202,
	);
	const proof = /confirmation#([a-f0-9]{64})/.exec(
		await mailText(email, "Confirm privileged operation"),
	)?.[1];
	assert.ok(proof);
	await page.goto("/v1/auth/reauth/confirmation#first");
	await page.waitForFunction(() => location.hash === "");
	await openReplacement(page, proof);
	assert.equal(await page.locator("#proof").textContent(), "");
	assert.equal(
		(await post(invitePath, { email: invitee, role: "viewer", proof }, false))
			.status,
		403,
	);
	await page
		.getByRole("button", { name: "Confirm and reveal proof", exact: true })
		.click();
	assert.equal(await page.locator("#proof").textContent(), proof);
	await openReplacement(page, "another-proof");
	assert.equal(await page.locator("#proof").textContent(), "");
	const invitation = await post(invitePath, {
		email: invitee,
		role: "viewer",
		proof,
	});
	assert.equal(invitation.status, 201);
	const invitationId = JSON.parse(invitation.text).id;
	assert.equal(
		(await post(invitePath, { email: invitee, role: "viewer", proof })).status,
		403,
	);
	assert.equal((await post("/v1/auth/confirm", { token })).status, 401);
	assert.equal(
		(await post("/v1/auth/request", { email, redirect: "https://evil.test" }))
			.status,
		422,
	);
	const owner = await browser.newContext({
		baseURL: origin,
		storageState: await context.storageState(),
	});
	const invitationText = await mailText(invitee, "Organization invitation");
	const invitationLink =
		/https?:\/\/[^\s]+\/v1\/invitations\/confirmation#[^\s]+/.exec(
			invitationText,
		)?.[0];
	assert.ok(invitationLink);
	const inviteUrl = new URL(invitationLink);
	assert.equal(inviteUrl.origin, origin);
	let acceptRequests = 0;
	page.on("request", (request) => {
		if (request.method() === "POST" && request.url().endsWith("/accept"))
			acceptRequests++;
	});
	await page.goto(`${inviteUrl.origin}${inviteUrl.pathname}#first:first:first`);
	await page.waitForFunction(() => location.hash === "");
	await openReplacement(page, inviteUrl.hash);
	assert.equal(await page.locator("#result").textContent(), "");
	await otherTab.goto("/v1/auth/confirmation");
	assert.equal(
		(await post("/v1/auth/request", { email: invitee })).status,
		202,
	);
	const inviteeLogin = /confirmation#([a-f0-9]{64})/.exec(
		await mailText(invitee, "Confirm sign in"),
	)?.[1];
	assert.ok(inviteeLogin);
	await otherTab.goto(`/v1/auth/confirmation#${inviteeLogin}`);
	await otherTab
		.getByRole("button", { name: "Confirm sign in", exact: true })
		.click();
	await otherTab.getByText("Signed in", { exact: true }).waitFor();
	assert.equal(acceptRequests, 0);
	const accepted = page.waitForResponse((response) =>
		response.url().endsWith(`/invitations/${invitationId}/accept`),
	);
	await page
		.getByRole("button", { name: "Accept invitation", exact: true })
		.click();
	assert.equal((await accepted).status(), 204);
	await page.getByText("Accepted", { exact: true }).waitFor();
	assert.equal(acceptRequests, 1);
	await openReplacement(page, "another:invitation:link");
	assert.equal(await page.locator("#result").textContent(), "");
	const memberships = await (
		await page.request.get("/v1/organizations")
	).json();
	assert.ok(
		memberships.some(
			(membership) => membership.id === org && membership.role === "viewer",
		),
	);
	await page.goto(`/#/organizations/${org}`);
	await page.getByRole("heading", { name: "Renamed project" }).waitFor();
	await page
		.getByText("You have view-only access to this organization.")
		.waitFor();
	assert.equal(
		await page.getByRole("button", { name: "Rename Renamed project" }).count(),
		0,
	);
	await checkTenantAccess({
		owner,
		viewer: context,
		origin,
		org,
		invitee,
		invitationId,
		invitationToken: inviteUrl.hash.split(":").at(-1),
		mailText: (subject) => mailText(email, subject),
	});
	const ownerPage = await owner.newPage();
	await ownerPage.goto(`/#/organizations/${org}`);
	await ownerPage.getByRole("heading", { name: "Renamed project" }).waitFor();
	await ownerPage.getByRole("button", { name: "Sign out" }).click();
	await ownerPage.getByLabel("Email address").waitFor();
	assert.equal(await ownerPage.getByText("Renamed project").count(), 0);
	console.log(
		"PASS: workspace sign-in, organization and project creation, persisted rename, sign-out, native cookie and confirmation flows",
	);
} finally {
	await browser.close();
}
