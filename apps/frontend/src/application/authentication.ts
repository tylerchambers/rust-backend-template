import { useMutation, useQuery } from "@tanstack/react-query";
import { useCallback, useEffect } from "react";
import { identityQueryKey } from "../infrastructure/authentication";
import { useAuthentication } from "./backend";

export function useIdentity() {
	const authentication = useAuthentication();
	const identity = useQuery({
		queryKey: identityQueryKey,
		queryFn: ({ signal }) => authentication.currentAccount(signal),
		retry: false,
		staleTime: 0,
		gcTime: 0,
		refetchOnMount: "always",
		refetchOnWindowFocus: "always",
		networkMode: "always",
	});
	const { refetch } = identity;
	// Query's default focus hook observes tab visibility. Also cover separate visible windows.
	useEffect(() => {
		const focus = (): void => {
			void refetch({ cancelRefetch: false });
		};
		window.addEventListener("focus", focus);
		return () => window.removeEventListener("focus", focus);
	}, [refetch]);
	const recheck = useCallback(async (): Promise<void> => {
		await refetch();
	}, [refetch]);
	return { identity, recheck };
}

export function useSignInLink(checking: boolean) {
	const authentication = useAuthentication();
	const request = useMutation({
		mutationFn: (email: string) => authentication.requestLink(email),
		retry: false,
		gcTime: 0,
		networkMode: "always",
	});
	function requestLink(email: string): void {
		if (!checking && !request.isPending) request.mutate(email);
	}
	return { request, requestLink };
}
