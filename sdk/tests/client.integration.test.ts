import { expect, test } from "bun:test";
import { z } from "zod";
import { createBackendClient } from "../src/index";

test("generated client logs in and creates a tenant project", async () => {
  const origin = "http://localhost:58039";
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
      request.headers.set("Cookie", [...cookies].map(([key, value]) => `${key}=${value}`).join("; "));
      return request;
    },
    onResponse({ response }) { captureCookies(response); return response; },
  });
  const email = `${crypto.randomUUID()}@example.test`;
  expect((await client.POST("/v1/auth/request", { body: { email } })).response.status).toBe(202);
  const listingSchema = z.object({ messages: z.array(z.object({ ID: z.string(), To: z.array(z.object({ Address: z.string() })) })) });
  const listing = listingSchema.parse(await (await fetch("http://127.0.0.1:58029/api/v1/messages")).json());
  const message = listing.messages.find((message) => message.To.some((to) => to.Address === email));
  if (!message) throw new Error("Login email missing");
  const content = z.object({ Text: z.string() }).parse(await (await fetch(`http://127.0.0.1:58029/api/v1/message/${message.ID}`)).json());
  const token = /confirmation#([a-f0-9]{64})/.exec(content.Text)?.[1];
  if (!token) throw new Error("Login token missing");
  expect((await client.POST("/v1/auth/confirm", { body: { token } })).response.status).toBe(200);
  const organization = await client.POST("/v1/organizations", { body: { name: "SDK tenant" } });
  if (!organization.data) throw new Error("Organization creation failed");
  const params = { path: { organization: organization.data.id } };
  const project = await client.POST("/v1/organizations/{organization}/projects", { params, body: { name: "SDK project" } });
  expect(project.response.status).toBe(201);
  const listingResult = await client.GET("/v1/organizations/{organization}/projects", { params });
  expect(listingResult.data?.[0]?.name).toBe("SDK project");
  expect((await client.POST("/v1/auth/logout")).response.status).toBe(204);
});
