import assert from "node:assert/strict";

const origin = process.env.PUBLIC_ORIGIN ?? "http://localhost:58039";
for (const path of ["/metrics", "/metrics/http"]) {
	const response = await fetch(new URL(path, origin));
	assert.equal(response.status, 200, path);
	assert.equal(
		response.headers.get("content-type"),
		"application/openmetrics-text; version=1.0.0; charset=utf-8",
	);
	const text = await response.text();
	assert.ok(text.endsWith("# EOF\n"));
	assert.match(text, /^# TYPE http_requests counter$/m);
	assert.match(text, /^# TYPE http_request_duration_seconds histogram$/m);
	if (path === "/metrics") {
		assert.match(text, /^# TYPE river_attempt_retries gauge$/m);
		assert.match(text, /^# TYPE db_pool_connections gauge$/m);
	}

	const buckets = [];
	const values = new Map();
	for (const line of text.trimEnd().split("\n")) {
		if (line.startsWith("#")) continue;
		// The only labels exposed are histogram bounds, never tenant or request data.
		const sample = /^(\w+)(?:\{le="([^"]+)"\})? (\S+)$/.exec(line);
		assert.ok(sample, `Unexpected metric or labels at ${path}`);
		const [, name, bound, raw] = sample;
		const value = Number(raw);
		assert.ok(Number.isFinite(value), name);
		if (bound !== undefined) {
			assert.equal(name, "http_request_duration_seconds_bucket");
			buckets.push({
				bound: bound === "+Inf" ? Infinity : Number(bound),
				value,
			});
		} else {
			values.set(name, value);
		}
	}
	const count = values.get("http_request_duration_seconds_count");
	assert.ok(count > 0 && values.get("http_request_duration_seconds_sum") > 0);
	assert.ok(buckets.length > 2);
	for (let i = 1; i < buckets.length; i++) {
		assert.ok(buckets[i].bound > buckets[i - 1].bound);
		assert.ok(buckets[i].value >= buckets[i - 1].value);
	}
	assert.deepEqual(buckets.at(-1), { bound: Infinity, value: count });
}

async function records(path) {
	assert.ok(path, "Expected server and worker log paths");
	const log = await Bun.file(path).text();
	assert.ok(
		!/[a-f0-9]{64}|@example\.test|confirmation#/i.test(log),
		`Secret or mailbox in ${path}`,
	);
	// Both processes are still running. Only inspect complete JSON records.
	return log
		.split("\n")
		.slice(0, -1)
		.map((line) => JSON.parse(line));
}
const [, , serverLog, workerLog] = process.argv;
const requests = new Map();
for (const { fields, span } of await records(serverLog)) {
	if (fields.operation === "http.request" && fields.status < 300) {
		requests.set(span.request_id, span.route);
	}
}
const correlated = new Set();
for (const { fields, span } of await records(workerLog)) {
	if (fields.operation !== "job.completed") continue;
	const route = requests.get(span.request_id);
	if (
		span.kind === "project_changed_v1" &&
		/^\/v1\/organizations\/\{organization\}\/projects(?:\/\{project\})?$/.test(
			route,
		)
	) {
		correlated.add(span.kind);
	}
	if (
		span.kind === "invitation_email_v1" &&
		route === "/v1/organizations/{organization}/invitations"
	) {
		correlated.add(span.kind);
	}
}
assert.deepEqual(
	correlated,
	new Set(["project_changed_v1", "invitation_email_v1"]),
);
console.log(
	"Both metric scrapes have cumulative buckets; HTTP requests correlate with worker jobs without logging secrets.",
);
