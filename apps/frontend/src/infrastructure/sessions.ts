import { infiniteQueryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { AuthenticationError } from "./authentication";
import { type WriteHeaders, writeHeaders } from "./browser";

const sessionPage = z.array(z.uuid()).max(50);
export const sessionListKey = ["workspace", "sessions"] as const;
export const sessionChangeKey = [...sessionListKey, "change"] as const;

function requireStatus(response: Response, expected: number): void {
	if (response.status === expected) return;
	switch (response.status) {
		case 401:
			throw new AuthenticationError("Your session has expired. Sign in again.");
		case 403:
			throw new AuthenticationError(
				"The security check failed. Check your session and try again.",
			);
		case 404:
			throw new AuthenticationError("This session is no longer available.");
		case 429:
			throw new AuthenticationError(
				"Too many requests. Wait a moment and try again.",
			);
		default:
			throw new AuthenticationError(
				"The service did not confirm the session request.",
			);
	}
}

export function sessionList(client: BackendClient) {
	return infiniteQueryOptions({
		queryKey: sessionListKey,
		initialPageParam: "",
		queryFn: async ({ pageParam, signal }) => {
			const { response, data } = await client.GET("/v1/auth/sessions", {
				params: { query: pageParam ? { after: pageParam } : {} },
				signal,
				cache: "no-store",
				redirect: "error",
			});
			requireStatus(response, 200);
			const page = sessionPage.safeParse(data);
			if (!page.success)
				throw new AuthenticationError(
					"The service returned invalid session IDs.",
				);
			return page.data;
		},
		getNextPageParam: (page) => page.at(-1),
	});
}

export type SessionChange =
	| { kind: "rotate" }
	| { kind: "revoke"; id: string }
	| { kind: "revokeAll" };

export async function changeSession(
	client: BackendClient,
	change: SessionChange,
	header: WriteHeaders = writeHeaders(),
): Promise<void> {
	const options = {
		params: { header },
		parseAs: "text",
		redirect: "error",
	} as const;
	switch (change.kind) {
		case "rotate":
			requireStatus(
				(await client.POST("/v1/auth/sessions/rotate", options)).response,
				204,
			);
			break;
		case "revokeAll":
			requireStatus(
				(await client.POST("/v1/auth/sessions/revoke-all", options)).response,
				204,
			);
			break;
		case "revoke":
			requireStatus(
				(
					await client.POST("/v1/auth/sessions/{session}/revoke", {
						...options,
						params: { header, path: { session: change.id } },
					})
				).response,
				204,
			);
			break;
	}
}
