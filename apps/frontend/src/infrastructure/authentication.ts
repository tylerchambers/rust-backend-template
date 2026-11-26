import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { type WriteHeaders, writeHeaders } from "./browser";

export const identityQueryKey = ["identity"] as const;

const accountSchema = z.object({ user_id: z.uuid(), email: z.string().min(1) });

export type Account = { id: string; email: string };

/** Only errors with deliberate, public-facing copy may be shown in the gate. */
export class AuthenticationError extends Error {}

export function authenticationMessage(
	error: unknown,
	fallback = "The service could not be reached. Check your connection and try again.",
): string {
	return error instanceof AuthenticationError ? error.message : fallback;
}

export class Authentication {
	readonly #client: BackendClient;
	readonly #headers: () => WriteHeaders;

	constructor(
		client: BackendClient,
		headers: () => WriteHeaders = writeHeaders,
	) {
		this.#client = client;
		this.#headers = headers;
	}

	async currentAccount(signal: AbortSignal): Promise<Account | null> {
		const { data, response } = await this.#client.GET("/v1/me", {
			cache: "no-store",
			redirect: "error",
			signal,
		});
		if (response.status === 401) return null;
		if (response.status !== 200) {
			throw new AuthenticationError(
				"The service could not verify your session.",
			);
		}
		const account = accountSchema.safeParse(data);
		if (!account.success) {
			throw new AuthenticationError(
				"The service returned invalid account details.",
			);
		}
		// The persistent session can outlive its browser-session CSRF cookie.
		await this.#prepareWrite(signal);
		return { id: account.data.user_id, email: account.data.email };
	}

	async requestLink(email: string): Promise<void> {
		const header = await this.#prepareWrite();
		const { response } = await this.#client.POST("/v1/auth/request", {
			params: { header },
			body: { email: email.trim() },
			parseAs: "text",
			redirect: "error",
		});
		switch (response.status) {
			case 202:
				return;
			case 400:
			case 422:
				throw new AuthenticationError("Check the email address and try again.");
			case 429:
				throw new AuthenticationError(
					"Too many requests. Wait a little before requesting another link.",
				);
			default:
				throw new AuthenticationError(
					"The sign-in link could not be requested. Please try again.",
				);
		}
	}

	async signOut(): Promise<void> {
		const header = await this.#prepareWrite();
		const { response } = await this.#client.POST("/v1/auth/logout", {
			params: { header },
			parseAs: "text",
			redirect: "error",
		});
		if (response.status !== 204 && response.status !== 401) {
			throw new AuthenticationError(
				"The service could not sign you out. Please try again.",
			);
		}
	}

	async #prepareWrite(signal?: AbortSignal): Promise<WriteHeaders> {
		let header = this.#headers();
		if (header["x-csrf-token"].length === 64) return header;

		// This GET sets a cookie. Its scanner-safe HTML is never rendered or executed here.
		const { response } = await this.#client.GET("/v1/auth/confirmation", {
			parseAs: "text",
			cache: "no-store",
			redirect: "error",
			signal: signal ?? null,
		});
		if (response.status !== 200) {
			throw new AuthenticationError(
				"The service could not prepare a secure request. Please try again.",
			);
		}
		header = this.#headers();
		if (header["x-csrf-token"].length !== 64) {
			throw new AuthenticationError("Allow cookies for this site to continue.");
		}
		return header;
	}
}
