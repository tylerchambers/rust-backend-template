import assert from "node:assert/strict";

// Reuse the browser journey's signed-in admin and member, including their real cookies.
async function tenantClient(context, origin) {
	const csrf = (await context.cookies()).find(
		(cookie) => cookie.name === "csrf",
	);
	assert.ok(csrf);
	const headers = { Origin: origin, "x-csrf-token": csrf.value };
	async function body(response, status) {
		assert.equal(response.status(), status, new URL(response.url()).pathname);
		if (response.headers()["content-type"]?.startsWith("application/json")) {
			return response.json();
		}
	}
	return {
		async get(path, status = 200) {
			return body(await context.request.get(path), status);
		},
		async post(path, data, status) {
			return body(await context.request.post(path, { headers, data }), status);
		},
	};
}

export async function checkTenantAccess({
	adminContext,
	memberContext,
	origin,
	org,
	invitee,
	invitationId,
	invitationToken,
	projectId,
}) {
	const admin = await tenantClient(adminContext, origin);
	const member = await tenantClient(memberContext, origin);
	const base = `/v1/organizations/${org}`;
	const projects = `${base}/projects`;
	const account = await member.get("/v1/me");
	assert.equal(account.email, invitee);
	await member.post(
		`${base}/invitations/${invitationId}/accept`,
		{ token: invitationToken },
		403,
	);
	const adminAccount = await admin.get("/v1/me");
	const adminPath = `${base}/members/${adminAccount.user_id}`;
	await admin.post(`${adminPath}/role`, { is_admin: false }, 409);
	await admin.post(`${adminPath}/remove`, undefined, 409);
	for (const [path, body] of [
		[`${base}/invitations`, { email: adminAccount.email, is_admin: true }],
		[`${base}/invitations/${invitationId}/revoke`, undefined],
		[`${base}/members/${account.user_id}/role`, { is_admin: true }],
		[`${adminPath}/remove`, undefined],
	]) {
		assert.deepEqual(await member.post(path, body, 403), { code: "forbidden" });
	}
	for (const list of ["invitations", "audit-events", "member-directory"]) {
		assert.deepEqual(await member.get(`${base}/${list}`, 403), {
			code: "forbidden",
		});
		for (const query of ["after=invalid", "offset=1"]) {
			assert.deepEqual(await admin.get(`${base}/${list}?${query}`, 400), {
				code: "invalid_input",
			});
		}
	}
	const [invitation] = await admin.get(`${base}/invitations`);
	assert.deepEqual(invitation, {
		id: invitationId,
		email: invitee,
		is_admin: false,
		status: "accepted",
		expires_at: invitation.expires_at,
	});
	assert.match(invitation.expires_at, /(?:Z|\+00:00)$/);
	assert.equal(
		(await member.get(`${projects}/${projectId}`)).name,
		"Renamed project",
	);
	const events = await admin.get(`${base}/audit-events`);
	const changed = events.find(
		(event) =>
			event.action === "project.updated" && event.subject === projectId,
	);
	assert.ok(changed);
	assert.deepEqual(Object.keys(changed).sort(), [
		"action",
		"actor_id",
		"id",
		"occurred_at",
		"subject",
	]);
	assert.equal(changed.actor_id, adminAccount.user_id);
	assert.match(changed.occurred_at, /(?:Z|\+00:00)$/);

	const other = await admin.post(
		"/v1/organizations",
		{ name: "Other tenant" },
		201,
	);
	const foreignPath = `/v1/organizations/${other.id}/projects`;
	const foreign = await admin.post(foreignPath, { name: "Foreign" }, 201);
	assert.deepEqual(await member.get(`${foreignPath}/${foreign.id}`, 403), {
		code: "forbidden",
	});
	for (const id of [foreign.id, crypto.randomUUID()]) {
		assert.deepEqual(await member.get(`${projects}/${id}`, 404), {
			code: "not_found",
		});
	}
	assert.deepEqual(await member.get(`${projects}/invalid`, 400), {
		code: "invalid_input",
	});
	for (const path of [projects, `${base}/members`, "/v1/organizations"]) {
		await admin.get(`${path}?after=invalid`, 400);
	}

	const ids = (await member.get(projects)).map((row) => row.id);
	assert.equal(ids.length, 2);
	for (let i = ids.length; i < 51; i++) {
		const row = await admin.post(projects, { name: `Page ${i}` }, 201);
		ids.push(row.id);
	}
	ids.sort();
	const first = await member.get(projects);
	assert.deepEqual(
		first.map((row) => row.id),
		ids.slice(0, 50),
	);
	const next = await member.get(`${projects}?after=${first.at(-1).id}`);
	assert.deepEqual(
		next.map((row) => row.id),
		ids.slice(50),
	);
	assert.deepEqual(await member.get(`${projects}?after=${next[0].id}`), []);
	const fromForeign = await member.get(`${projects}?after=${foreign.id}`);
	assert.deepEqual(
		fromForeign.map((row) => row.id),
		ids.filter((id) => id > foreign.id).slice(0, 50),
	);

	const team = await adminContext.newPage();
	await team.goto(`/#/organizations/${org}/settings`);
	await team
		.getByText("The organization must keep at least one admin.")
		.waitFor();
	await team
		.getByLabel(`Change role for ${invitee}`)
		.selectOption({ label: "Admin" });
	await team
		.getByRole("dialog", { name: "Change role" })
		.getByRole("button", { name: "Change role", exact: true })
		.click();
	await team
		.getByRole("table", { name: "Members", exact: true })
		.getByRole("row")
		.filter({ hasText: invitee })
		.getByRole("cell", { name: "Admin", exact: true })
		.waitFor();
	await team.close();
	const directory = await admin.get(`${base}/member-directory`);
	assert.deepEqual(
		directory.find((member) => member.user_id === account.user_id),
		{ ...account, is_admin: true },
	);
	assert.deepEqual(await member.get("/v1/me", 401), {
		code: "unauthenticated",
	});
}
