import { createContext, useContext } from "react";
import type { BackendClient } from "rust-backend-template-sdk";
import type { Authentication } from "../infrastructure/authentication";

type Backend = { client: BackendClient; authentication: Authentication };
export const BackendContext = createContext<Backend | null>(null);

function useBackendServices(): Backend {
	const backend = useContext(BackendContext);
	if (!backend) throw new Error("Backend provider is missing");
	return backend;
}

export function useBackend(): BackendClient {
	return useBackendServices().client;
}

export function useAuthentication(): Authentication {
	return useBackendServices().authentication;
}
