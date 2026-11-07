import createClient from "openapi-fetch";
import type { paths } from "./schema";

/** Fetch-based typed SDK. Supply CSRF and cookie handling for non-browser clients. */
export function createBackendClient(baseUrl: string, csrfToken: () => string) {
  const client = createClient<paths>({ baseUrl, credentials: "include" });
  client.use({ onRequest({ request }) {
    if (!["GET", "HEAD", "OPTIONS"].includes(request.method)) {
      request.headers.set("x-csrf-token", csrfToken());
    }
    return request;
  } });
  return client;
}
export type { paths } from "./schema";
