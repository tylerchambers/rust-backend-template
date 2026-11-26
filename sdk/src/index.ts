import createClient, { type Client } from "openapi-fetch";
import type { paths } from "./schema";

export type BackendClient = Client<paths>;

/** Fetch-based typed SDK. Supply Origin/CSRF parameters and cookies for non-browser clients.
 * Confirmation pages are HTML: use GET(path, { parseAs: "text" }).
 */
export function createBackendClient(
	baseUrl: string,
	csrfToken: () => string,
): BackendClient {
	const client = createClient<paths>({ baseUrl, credentials: "include" });
	client.use({
		onRequest({ request }) {
			if (!["GET", "HEAD", "OPTIONS"].includes(request.method)) {
				request.headers.set("x-csrf-token", csrfToken());
			}
			return request;
		},
	});
	return client;
}
export type { paths } from "./schema";
