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

async function emailedSignInUrl(email) {
	const text = await mailText(email, "Confirm sign in");
	const link = /^https?:\/\/\S+/m.exec(text)?.[0];
	assert.ok(link, "The sign-in email must include a link");
	const url = new URL(link);
	assert.equal(url.origin, origin);
	assert.equal(url.pathname, "/sign-in");
	assert.equal(url.search, "");
	assert.ok(/^#[a-f0-9]{64}$/.test(url.hash));
	return url;
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
	const login = await emailedSignInUrl(email);
	await page.goto(login.href);
	await page.waitForURL(`${origin}/sign-in`);
	assert.equal((await page.request.get("/v1/auth/sessions")).status(), 401);
	const scannerTab = await context.newPage();
	// An older email still opens the confirmation view and rotates the shared CSRF cookie.
	await scannerTab.goto(`/v1/auth/confirmation${login.hash}`);
	await scannerTab.waitForURL(`${origin}/sign-in`);
	const scannerConfirm = scannerTab.getByRole("button", {
		name: "Confirm sign in",
		exact: true,
	});
	await scannerConfirm.waitFor();
	assert.ok(await scannerConfirm.isEnabled());
	await openReplacement(scannerTab, "0".repeat(64));
	const rejected = scannerTab.waitForResponse(`${origin}/v1/auth/confirm`);
	await scannerConfirm.click();
	assert.equal((await rejected).status(), 401);
	await scannerTab
		.getByRole("alert")
		.getByText(/invalid or has expired/i)
		.waitFor();
	await scannerTab.locator('main a[href="/"]').click();
	await scannerTab.getByLabel("Email address").waitFor();
	await scannerTab.close();
	assert.equal((await page.request.get("/v1/auth/sessions")).status(), 401);
	const confirmationResponse = page.waitForResponse((response) =>
		response.url().endsWith("/v1/auth/confirm"),
	);
	await page
		.getByRole("button", { name: "Confirm sign in", exact: true })
		.click();
	const response = await confirmationResponse;
	assert.equal(response.status(), 200);
	await page.getByText(email, { exact: true }).waitFor();
	assert.equal(new URL(page.url()).pathname, "/");
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
	const otherTab = await context.newPage();
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
	// Hold the write until the editor unmounts and its replacement list has refetched.
	const projectsUrl = `${origin}/v1/organizations/${org}/projects`;
	const pendingRename = Promise.withResolvers();
	await page.route(
		`${projectsUrl}/*`,
		(route) => pendingRename.resolve(route),
		{
			times: 1,
		},
	);
	await page.getByRole("button", { name: "Save name" }).click();
	const timeout = setTimeout(() => {
		pendingRename.reject(new Error("Timed out waiting for the project PATCH"));
	}, 5000);
	const renameRoute = await pendingRename.promise.finally(() =>
		clearTimeout(timeout),
	);
	assert.equal(renameRoute.request().method(), "PATCH");
	await page.getByRole("tab", { name: "Members" }).click();
	const listRefetched = page.waitForResponse(projectsUrl);
	await page.getByRole("tab", { name: "Projects" }).click();
	await (await listRefetched).finished();
	await page
		.getByRole("region", { name: "Organization projects" })
		.and(page.locator('[aria-busy="false"]'))
		.getByRole("heading", { name: "Another editor's project" })
		.waitFor();
	await renameRoute.continue();
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
	await page.getByRole("tab", { name: "Invitations" }).click();
	await page.getByLabel("Recipient email").fill(invitee);
	await page.getByLabel("Invitation role").selectOption("viewer");
	await page.getByRole("button", { name: "Review invitation" }).click();
	const proofRequest = page.waitForResponse((response) =>
		response.url().endsWith("/v1/auth/reauth/request"),
	);
	await page.getByRole("button", { name: "Email me a proof" }).click();
	assert.equal((await proofRequest).status(), 202);
	const proof = /confirmation#([a-f0-9]{64})/.exec(
		await mailText(email, "Confirm privileged operation"),
	)?.[1];
	assert.ok(proof);
	await otherTab.goto("/v1/auth/reauth/confirmation#first");
	await otherTab.waitForFunction(() => location.hash === "");
	await openReplacement(otherTab, proof);
	assert.equal(await otherTab.locator("#proof").textContent(), "");
	assert.equal(
		(await post(invitePath, { email: invitee, role: "viewer", proof }, false))
			.status,
		403,
	);
	await otherTab
		.getByRole("button", { name: "Confirm and reveal proof", exact: true })
		.click();
	const revealed = await otherTab.locator("#proof").textContent();
	assert.equal(revealed, proof);
	await openReplacement(otherTab, "another-proof");
	assert.equal(await otherTab.locator("#proof").textContent(), "");
	const sent = page.waitForResponse(
		(response) =>
			response.request().method() === "POST" &&
			response.url().endsWith(invitePath),
	);
	await page.getByLabel("Email proof").fill(revealed);
	await page
		.getByRole("button", { name: "Send invitation", exact: true })
		.click();
	const invitation = await sent;
	assert.equal(invitation.status(), 201);
	const { id: invitationId } = await invitation.json();
	await page
		.getByRole("article")
		.filter({ hasText: invitee })
		.getByText("pending")
		.waitFor();
	assert.equal(
		(await post(invitePath, { email: invitee, role: "viewer", proof })).status,
		403,
	);
	assert.equal(
		(await post("/v1/auth/confirm", { token: login.hash.slice(1) })).status,
		401,
	);
	assert.equal(
		(await post("/v1/auth/request", { email, redirect: "https://evil.test" }))
			.status,
		422,
	);
	const owner = await browser.newContext({
		baseURL: origin,
		storageState: await context.storageState(),
	});
	const workspaceTab = await context.newPage();
	await workspaceTab.goto("/");
	const ownerOrganization = workspaceTab.getByRole("link", {
		name: "Browser proof tenant",
		exact: true,
	});
	await ownerOrganization.waitFor();
	await workspaceTab.getByLabel("Organization name").fill("Owner's draft");
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
	const inviteeLogin = await emailedSignInUrl(invitee);
	await otherTab.goto(inviteeLogin.href);
	await otherTab.waitForURL(`${origin}/sign-in`);
	const beforeSwitch = await otherTab.request.get("/v1/me");
	assert.equal(beforeSwitch.status(), 200);
	assert.equal((await beforeSwitch.json()).email, email);
	await otherTab
		.getByRole("button", { name: "Confirm sign in", exact: true })
		.click();
	await otherTab.getByText(invitee, { exact: true }).waitFor();
	assert.equal(new URL(otherTab.url()).pathname, "/");
	// A still-open workspace must discard the previous account's data and forms on focus.
	await workspaceTab.evaluate(() => window.dispatchEvent(new Event("focus")));
	await workspaceTab.getByText(invitee, { exact: true }).waitFor();
	assert.equal(
		await workspaceTab.getByLabel("Organization name").inputValue(),
		"",
	);
	assert.equal(await ownerOrganization.count(), 0);
	await workspaceTab.close();
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
	await page.getByRole("tab", { name: "Members" }).click();
	const me = await (await page.request.get("/v1/me")).json();
	const roster = page.getByRole("region", { name: "Organization members" });
	await roster.getByText(me.user_id).waitFor();
	assert.equal(await roster.getByText(email, { exact: true }).count(), 0);
	assert.equal(await page.getByRole("tab", { name: "Invitations" }).count(), 0);
	assert.equal(
		await page.getByRole("tab", { name: "Audit events" }).count(),
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
	await ownerPage.getByRole("tab", { name: "Audit events" }).click();
	await ownerPage.getByRole("button", { name: "Load more events" }).click();
	await ownerPage
		.getByText("membership.role_changed", { exact: true })
		.waitFor();
	await ownerPage.getByRole("tab", { name: "Invitations" }).click();
	await ownerPage
		.getByRole("article")
		.filter({ hasText: invitee })
		.getByText("accepted")
		.waitFor();
	await ownerPage.getByRole("button", { name: "Manage sessions" }).click();
	await ownerPage.getByRole("button", { name: /^Revoke session / }).waitFor();
	await ownerPage
		.getByRole("button", { name: "Rotate current session" })
		.click();
	await ownerPage.getByRole("button", { name: "Confirm rotation" }).click();
	await ownerPage.getByText("Current session rotated.").waitFor();
	await ownerPage.getByRole("tab", { name: "Projects" }).click();
	await ownerPage.getByRole("heading", { name: "Renamed project" }).waitFor();
	await ownerPage.getByRole("button", { name: "Sign out" }).click();
	await ownerPage.getByLabel("Email address").waitFor();
	assert.equal(await ownerPage.getByText("Renamed project").count(), 0);
	console.log(
		"PASS: workspace projects, session rotation, role-aware roster, audit history, email proof and invitation lifecycle",
	);
} finally {
	await browser.close();
}
