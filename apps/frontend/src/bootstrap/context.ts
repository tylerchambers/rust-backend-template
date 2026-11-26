import { createContext, useContext } from "react";
import type { BackendClient } from "rust-backend-template-sdk";

export const BackendContext = createContext<BackendClient | null>(null);

export function useBackend(): BackendClient {
	const backend = useContext(BackendContext);
	if (!backend) throw new Error("Backend provider is missing");
	return backend;
}
