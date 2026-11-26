import { beforeEach, expect, test } from "bun:test";
import { $ } from "bun";

const container = process.env.TEST_POSTGRES_CONTAINER;
if (!container) throw new Error("Run seed tests with just integration");
const seed = await Bun.file(
	new URL("../../dev/seed.sql", import.meta.url),
).text();
const owner = "00000000-0000-7000-8000-000000000001";
const north = "00000000-0000-7000-8000-000000000101";
const project = "00000000-0000-7000-8000-000000001001";

// The recipe clones the migrated database before any fixture rows exist.
function sql(source) {
	return $`docker exec -i ${container} psql -XqAt -v ON_ERROR_STOP=1 -U postgres -d seed_test < ${Buffer.from(source)}`.quiet();
}

function snapshot() {
	return sql(`
		SELECT row_to_json(u) FROM users u ORDER BY id;
		SELECT row_to_json(o) FROM organizations o ORDER BY id;
		SELECT row_to_json(m) FROM memberships m ORDER BY organization_id, user_id;
		SELECT row_to_json(p) FROM projects p ORDER BY id;
	`).text();
}

beforeEach(async () => {
	await sql(`
		TRUNCATE users, organizations CASCADE;
		INSERT INTO users(id,email) VALUES (gen_random_uuid(),'unrelated@example.test');
		INSERT INTO organizations(id,name) VALUES (gen_random_uuid(),'Unrelated tenant');
	`);
});

test("rerunning the seed leaves both the example and unrelated rows unchanged", async () => {
	await sql(seed);
	const before = await snapshot();
	expect(before).toContain("unrelated@example.test");
	expect(before).toContain("Unrelated tenant");
	expect(before).toContain("owner@example.test");
	expect(before).toContain("North example");
	expect(before).toContain("South example");
	expect(
		(
			await sql("SELECT count(*) FROM memberships WHERE role='owner'").text()
		).trim(),
	).toBe("2");
	await sql(seed);
	expect(await snapshot()).toBe(before);
});

test("a conflicting user prevents publication of new organizations and ownership", async () => {
	await sql(
		`INSERT INTO users(id,email) VALUES ('${owner}', 'conflict@example.test')`,
	);
	const before = await snapshot();
	const result = await sql(seed).nothrow();
	expect(result.exitCode).not.toBe(0);
	expect(result.stderr.toString()).toContain("Seed conflict");
	expect(await snapshot()).toBe(before);
});

for (const [table, change] of [
	["users", `UPDATE users SET email='other@example.test' WHERE id='${owner}'`],
	[
		"organizations",
		`UPDATE organizations SET name='Other' WHERE id='${north}'`,
	],
	["projects", `UPDATE projects SET name='Other' WHERE id='${project}'`],
	[
		"memberships",
		`UPDATE memberships SET role='member' WHERE organization_id='${north}'`,
	],
]) {
	test(`the seed rejects conflicting ${table} without changing existing rows`, async () => {
		await sql(seed);
		await sql(change);
		const before = await snapshot();
		const result = await sql(seed).nothrow();
		expect(result.exitCode).not.toBe(0);
		expect(result.stderr.toString()).toContain("Seed conflict");
		expect(await snapshot()).toBe(before);
	});
}

test("terminating the seed backend during a project insert rolls back ownership", async () => {
	const before = await snapshot();
	await sql(`
		CREATE FUNCTION interrupt_seed() RETURNS trigger LANGUAGE plpgsql AS $$
		BEGIN PERFORM pg_terminate_backend(pg_backend_pid()); RETURN NEW; END $$;
		CREATE TRIGGER interrupt_seed BEFORE INSERT ON projects FOR EACH ROW EXECUTE FUNCTION interrupt_seed();
	`);
	try {
		const result = await sql(seed).nothrow();
		expect(result.exitCode).not.toBe(0);
		expect(result.stderr.toString()).toContain("terminating connection");
		expect(await snapshot()).toBe(before);
	} finally {
		await sql(
			"DROP TRIGGER interrupt_seed ON projects; DROP FUNCTION interrupt_seed()",
		);
	}
});
