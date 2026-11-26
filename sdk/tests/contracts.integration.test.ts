import { expect, test } from "bun:test";
import { createBackendClient } from "../src/index";

test("typed errors and HTML pages follow the published contract", async () => {
	const origin = process.env.PUBLIC_ORIGIN ?? "http://localhost:58039";
	const client = createBackendClient(origin, () => "");
	const sessions = await client.GET("/v1/auth/sessions");
	expect(sessions.response.status).toBe(401);
	expect(sessions.error?.code).toBe("unauthenticated");
	const organization = await client.POST("/v1/organizations", {
		params: {
			header: { Origin: origin, "x-csrf-token": "" },
		},
		body: { name: "Forbidden" },
	});
	expect(organization.response.status).toBe(403);
	expect(organization.error?.code).toBe("forbidden");
	for (const path of [
		"/v1/auth/confirmation",
		"/v1/invitations/confirmation",
	] as const) {
		const page = await client.GET(path, { parseAs: "text" });
		expect(page.response.headers.get("content-type")).toContain("text/html");
		expect(page.data?.startsWith("<!doctype html>")).toBe(true);
	}
});
