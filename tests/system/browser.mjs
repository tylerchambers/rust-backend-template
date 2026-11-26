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

async function assertFitsViewport(page) {
	assert.ok(
		await page.evaluate(
			() => document.documentElement.scrollWidth <= window.innerWidth,
		),
		"Page should not require horizontal scrolling",
	);
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
	await scannerTab.setViewportSize({ width: 320, height: 720 });
	await assertFitsViewport(scannerTab);
	await scannerTab.keyboard.press("Tab");
	await scannerTab.keyboard.press("Enter");
	assert.ok(
		await scannerTab
			.getByRole("main")
			.evaluate((el) => el === document.activeElement),
	);
	assert.ok(await scannerConfirm.isEnabled());
	await openReplacement(scannerTab, "0".repeat(64));
	const rejected = scannerTab.waitForResponse(`${origin}/v1/auth/confirm`);
	await scannerConfirm.click();
	assert.equal((await rejected).status(), 401);
	await scannerTab
		.getByRole("alert")
		.getByText(/invalid or has expired/i)
		.waitFor();
	await scannerTab.getByRole("link", { name: "Back to Project desk" }).click();
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
	const table = page.getByRole("table", { name: "Active organizations" });
	await table.getByText("No organizations yet.").waitFor();
	await page.getByRole("link", { name: "New organization" }).click();
	await page.waitForURL(`${origin}/#/organizations/new`);
	await page.getByLabel("Organization name").fill("Browser tenant");
	const organization = page.waitForResponse(
		(response) =>
			response.request().method() === "POST" &&
			response.url().endsWith("/v1/organizations"),
	);
	await page.getByRole("button", { name: "Create organization" }).click();
	const created = await organization;
	assert.equal(created.status(), 201);
	const { id: org } = await created.json();
	const organizationCrumb = (tab) =>
		tab.getByRole("navigation", { name: "Breadcrumb" })
			.locator(`a[href$="/organizations/${org}"]`);
	await page.waitForURL(`${origin}/#/`);
	await table.getByRole("row", { name: /Browser tenant Admin/i }).waitFor();
	await table
		.getByRole("link", { name: "Browser tenant", exact: true })
		.click();
	const projectsTable = page.getByRole("table", { name: "Projects" });
	await projectsTable.getByText("No projects yet.").waitFor();
	await page.getByRole("link", { name: "New project" }).click();
	await page.waitForURL(`${origin}/#/organizations/${org}/projects/new`);
	await page.getByRole("link", { name: "Cancel" }).click();
	await projectsTable.waitFor();
	await page.getByRole("link", { name: "New project" }).click();
	await page.getByLabel("Project name").fill("First project");
	const projectsUrl = `${origin}/v1/organizations/${org}/projects`;
	const firstProject = page.waitForResponse(
		(response) =>
			response.url() === projectsUrl && response.request().method() === "POST",
	);
	await page.getByRole("button", { name: "Create project" }).click();
	const projectResponse = await firstProject;
	assert.equal(projectResponse.status(), 201);
	const { id: projectId } = await projectResponse.json();
	await page.waitForURL(`${origin}/#/organizations/${org}`);
	await projectsTable.getByRole("link", { name: "First project" }).click();
	await page.getByRole("button", { name: "Rename project" }).click();
	const otherTab = await context.newPage();
	await otherTab.goto(page.url());
	await otherTab.getByRole("button", { name: "Rename project" }).click();
	await otherTab
		.getByLabel("New project name")
		.fill("Another editor's project");
	await otherTab.getByRole("button", { name: "Save name" }).click();
	await otherTab
		.getByRole("heading", { name: "Another editor's project", exact: true })
		.waitFor();
	await page.getByLabel("New project name").fill("Renamed project");
	await page.getByRole("button", { name: "Save name" }).click();
	await page.getByRole("alert").getByText("Another editor's project").waitFor();
	assert.equal(
		await page.getByLabel("New project name").inputValue(),
		"Renamed project",
	);
	await page
		.getByRole("button", { name: "Continue with this version" })
		.click();
	// Hold the write until the editor unmounts and its replacement list has refetched.
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
	const listRefetched = page.waitForResponse(projectsUrl);
	await organizationCrumb(page).click();
	await (await listRefetched).finished();
	await page
		.getByRole("region", { name: "Organization projects" })
		.and(page.locator('[aria-busy="false"]'))
		.getByRole("cell", { name: "Another editor's project", exact: true })
		.waitFor();
	await renameRoute.continue();
	const renamed = { name: "Renamed project", exact: true };
	await projectsTable.getByRole("link", renamed).click();
	await page.waitForURL(
		`${origin}/#/organizations/${org}/projects/${projectId}`,
	);
	await page.reload();
	await page.getByRole("heading", renamed).waitFor();
	await organizationCrumb(page).click();
	await projectsTable.getByRole("link", renamed).waitFor();
	const invitePath = `/v1/organizations/${org}/invitations`;
	const invitee = `${randomUUID()}@example.test`;
	assert.equal(
		(await post(invitePath, { email: invitee, is_admin: false }, false)).status,
		403,
	);
	const settingsLink = { name: "Organization settings", exact: true };
	await page.getByRole("link", settingsLink).click();
	await page.waitForURL(`${origin}/#/organizations/${org}/settings`);
	await page.reload();
	const inviteMember = page.getByRole("link", { name: "Invite member" });
	await inviteMember.click();
	await page.waitForURL(`${origin}/#/organizations/${org}/invitations/new`);
	await page.getByRole("link", { name: "Cancel" }).click();
	await page.getByRole("table", { name: "Members", exact: true }).waitFor();
	await inviteMember.click();
	await page.getByLabel("Recipient email").fill(invitee);
	await page.getByRole("button", { name: "Review invitation" }).click();
	const sent = page.waitForResponse(
		(response) =>
			response.request().method() === "POST" &&
			response.url().endsWith(invitePath),
	);
	await page
		.getByRole("dialog", { name: "Send invitation" })
		.getByRole("button", { name: "Send invitation", exact: true })
		.click();
	const invitation = await sent;
	assert.equal(invitation.status(), 201);
	assert.deepEqual(invitation.request().postDataJSON(), {
		email: invitee,
		is_admin: false,
	});
	const { id: invitationId } = await invitation.json();
	const pendingInvite = page
		.getByRole("table", { name: "Invitations" })
		.getByRole("row")
		.filter({ hasText: invitee });
	await pendingInvite.getByText("pending").waitFor();
	await pendingInvite.getByRole("button", { name: /Revoke/ }).click();
	await page
		.getByRole("dialog", { name: "Revoke invitation" })
		.getByRole("button", { name: "Cancel" })
		.click();
	await pendingInvite.getByText("pending").waitFor();
	assert.equal(
		(await post("/v1/auth/confirm", { token: login.hash.slice(1) })).status,
		401,
	);
	assert.equal(
		(await post("/v1/auth/request", { email, redirect: "https://evil.test" }))
			.status,
		422,
	);
	const adminContext = await browser.newContext({
		baseURL: origin,
		viewport: { width: 320, height: 720 },
		storageState: await context.storageState(),
	});
	const workspaceTab = await context.newPage();
	await workspaceTab.goto("/");
	const adminOrganization = workspaceTab.getByRole("link", {
		name: "Browser tenant",
		exact: true,
	});
	await adminOrganization.waitFor();
	await workspaceTab.getByRole("link", { name: "New organization" }).click();
	await workspaceTab.getByLabel("Organization name").fill("Admin's draft");
	const invitationText = await mailText(invitee, "Organization invitation");
	const invitationLink =
		/https?:\/\/[^\s]+\/v1\/invitations\/confirmation#[^\s]+/.exec(
			invitationText,
		)?.[0];
	assert.ok(invitationLink);
	const inviteUrl = new URL(invitationLink);
	assert.equal(inviteUrl.origin, origin);
	const invitationToken = inviteUrl.hash.split(":").at(-1);
	assert.equal(
		(
			await post(`${invitePath}/${invitationId}/accept`, {
				token: invitationToken,
			})
		).status,
		403,
	);
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
	await workspaceTab.getByRole("link", { name: "Cancel" }).click();
	await workspaceTab
		.getByRole("table", { name: "Active organizations" })
		.waitFor();
	assert.equal(await adminOrganization.count(), 0);
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
			(membership) => membership.id === org && membership.is_admin === false,
		),
	);
	await page.goto(`/#/organizations/${org}`);
	await projectsTable.getByRole("link", renamed).click();
	await page.waitForURL(
		`${origin}/#/organizations/${org}/projects/${projectId}`,
	);
	await page.getByRole("heading", renamed).waitFor();
	await organizationCrumb(page).click();
	await page.getByRole("link", { name: "New project" }).click();
	await page.getByLabel("Project name").fill("Member project");
	await page.getByRole("button", { name: "Create project" }).click();
	await projectsTable.getByRole("link", { name: "Member project" }).click();
	await page.getByRole("button", { name: "Rename project" }).click();
	await page.getByLabel("New project name").fill("Member's update");
	await page.getByRole("button", { name: "Save name" }).click();
	await page.getByRole("heading", { name: "Member's update" }).waitFor();
	await organizationCrumb(page).click();
	await projectsTable.getByRole("link", { name: "Member's update" }).waitFor();
	await page.getByRole("link", settingsLink).click();
	const me = await (await page.request.get("/v1/me")).json();
	const roster = page.getByRole("region", { name: "Organization members" });
	await roster.getByText(me.user_id).waitFor();
	assert.equal(await roster.getByText(email, { exact: true }).count(), 0);
	assert.equal(await inviteMember.count(), 0);
	assert.equal(
		await page.getByRole("table", { name: "Invitations" }).count(),
		0,
	);
	assert.equal(
		await page.getByRole("tab", { name: "Audit events" }).count(),
		0,
	);
	await page.goto(`/#/organizations/${org}/invitations/new`);
	await page.getByRole("navigation", { name: "Breadcrumb" })
		.getByRole("link", { name: "Settings" }).waitFor();
	assert.equal(await page.getByLabel("Recipient email").count(), 0);
	await checkTenantAccess({
		adminContext,
		memberContext: context,
		origin,
		org,
		invitee,
		invitationId,
		invitationToken,
		projectId,
	});
	const adminPage = await adminContext.newPage();
	await adminPage.goto(`/#/organizations/${org}`);
	const projectLinks = adminPage
		.getByRole("table", { name: "Projects" })
		.getByRole("link");
	await projectLinks.first().waitFor();
	await assertFitsViewport(adminPage);
	await adminPage.getByRole("link", settingsLink).click();
	await adminPage.getByRole("tab", { name: "Audit events" }).click();
	await adminPage.reload();
	const eventList = adminPage.getByRole("list", { name: "Events" });
	await eventList
		.getByRole("listitem")
		.first()
		.getByText("membership.role_changed", { exact: true })
		.waitFor();
	await adminPage.getByRole("button", { name: "Load more events" }).click();
	await eventList
		.getByRole("listitem")
		.last()
		.getByText("organization.created", { exact: true })
		.waitFor();
	await assertFitsViewport(adminPage);
	await adminPage.getByRole("tab", { name: "Members" }).click();
	await adminPage
		.getByRole("table", { name: "Invitations" })
		.getByRole("row")
		.filter({ hasText: invitee })
		.getByText("accepted")
		.waitFor();
	await assertFitsViewport(adminPage);
	const header = adminPage.getByRole("banner");
	const profile = header.getByRole("button", { name: `Profile: ${email}` });
	const menu = adminPage.getByRole("menu");
	const sessions = adminPage.getByRole("region", {
		name: "Sessions",
		exact: true,
	});
	assert.equal(await sessions.count(), 0);
	await profile.click();
	await menu.getByRole("menuitem", { name: "Account settings" }).click();
	await adminPage.waitForURL(`${origin}/#/account/settings`);
	await adminPage.reload();
	await adminPage
		.getByRole("heading", { level: 1, name: "Account settings" })
		.waitFor();
	assert.equal(
		await adminPage
			.getByRole("region", { name: "Organization members" })
			.count(),
		0,
	);
	await sessions.getByRole("button", { name: /^Revoke session / }).waitFor();
	await sessions
		.getByRole("button", { name: "Rotate current session" })
		.click();
	await sessions.getByRole("button", { name: "Confirm rotation" }).click();
	await sessions.getByText("Current session rotated.").waitFor();
	await assertFitsViewport(adminPage);
	await adminPage.goBack();
	await organizationCrumb(adminPage).click();
	await projectLinks.first().waitFor();
	await profile.click();
	await menu.getByRole("menuitem", { name: "Log out" }).click();
	await adminPage.getByLabel("Email address").waitFor();
	assert.equal(await profile.count(), 0);
	assert.equal(await adminPage.getByText("Renamed project").count(), 0);
	console.log(
		"PASS: admin and member project editing, session rotation, roster permissions, audit history and emailed invitation acceptance",
	);
} finally {
	await browser.close();
}
