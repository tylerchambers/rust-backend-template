import assert from "node:assert/strict";

// Reuse the browser journey's signed-in owner and viewer, including their real cookies.
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
	owner,
	viewer,
	origin,
	org,
	invitee,
	invitationId,
	invitationToken,
	mailText,
}) {
	const admin = await tenantClient(owner, origin);
	const guest = await tenantClient(viewer, origin);
	const base = `/v1/organizations/${org}`;
	const projects = `${base}/projects`;
	const account = await guest.get("/v1/me");
	assert.equal(account.email, invitee);
	await guest.post(
		`${base}/invitations/${invitationId}/accept`,
		{ token: invitationToken },
		403,
	);
	await guest.post(projects, { name: "Not allowed" }, 403);
	for (const list of ["invitations", "audit-events", "member-directory"]) {
		assert.deepEqual(await guest.get(`${base}/${list}`, 403), {
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
		role: "viewer",
		status: "accepted",
		expires_at: invitation.expires_at,
	});
	assert.match(invitation.expires_at, /(?:Z|\+00:00)$/);
	const [project] = await guest.get(projects);
	assert.equal(
		(await guest.get(`${projects}/${project.id}`)).name,
		"Renamed project",
	);
	const events = await admin.get(`${base}/audit-events`);
	const changed = events.find(
		(event) =>
			event.action === "project.updated" && event.subject === project.id,
	);
	assert.ok(changed);
	assert.deepEqual(Object.keys(changed).sort(), [
		"action",
		"actor_id",
		"id",
		"occurred_at",
		"subject",
	]);
	assert.equal(changed.actor_id, (await admin.get("/v1/me")).user_id);
	assert.match(changed.occurred_at, /(?:Z|\+00:00)$/);

	const other = await admin.post(
		"/v1/organizations",
		{ name: "Other tenant" },
		201,
	);
	const foreignPath = `/v1/organizations/${other.id}/projects`;
	const foreign = await admin.post(foreignPath, { name: "Foreign" }, 201);
	assert.deepEqual(await guest.get(`${foreignPath}/${foreign.id}`, 403), {
		code: "forbidden",
	});
	for (const id of [foreign.id, crypto.randomUUID()]) {
		assert.deepEqual(await guest.get(`${projects}/${id}`, 404), {
			code: "not_found",
		});
	}
	assert.deepEqual(await guest.get(`${projects}/invalid`, 400), {
		code: "invalid_input",
	});
	for (const path of [projects, `${base}/members`, "/v1/organizations"]) {
		await admin.get(`${path}?after=invalid`, 400);
	}

	const ids = [project.id];
	for (let i = 0; i < 50; i++) {
		const row = await admin.post(projects, { name: `Page ${i}` }, 201);
		ids.push(row.id);
	}
	ids.sort();
	const first = await guest.get(projects);
	assert.deepEqual(
		first.map((row) => row.id),
		ids.slice(0, 50),
	);
	const next = await guest.get(`${projects}?after=${first.at(-1).id}`);
	assert.deepEqual(
		next.map((row) => row.id),
		ids.slice(50),
	);
	assert.deepEqual(await guest.get(`${projects}?after=${next[0].id}`), []);
	const fromForeign = await guest.get(`${projects}?after=${foreign.id}`);
	assert.deepEqual(
		fromForeign.map((row) => row.id),
		ids.filter((id) => id > foreign.id).slice(0, 50),
	);

	await admin.post(
		"/v1/auth/reauth/request",
		{
			action: "change_role",
			target: account.user_id,
			organization_id: org,
		},
		202,
	);
	const proof = /confirmation#([a-f0-9]{64})/.exec(
		await mailText("Confirm privileged operation"),
	)?.[1];
	assert.ok(proof);
	await admin.post(
		`${base}/members/${account.user_id}/role`,
		{ role: "member", proof },
		204,
	);
	const directory = await admin.get(`${base}/member-directory`);
	assert.deepEqual(
		directory.find((member) => member.user_id === account.user_id),
		{ ...account, role: "member" },
	);
	assert.deepEqual(await guest.get("/v1/me", 401), { code: "unauthenticated" });
}
