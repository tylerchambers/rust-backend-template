import {
	useInfiniteQuery,
	useMutation,
	useQueryClient,
} from "@tanstack/react-query";
import { useState } from "react";
import {
	changeSession,
	type SessionChange,
	sessionChangeKey,
	sessionList,
} from "../infrastructure/sessions";
import { useAccountSession } from "./accountSession";
import { useBackend } from "./backend";

export function useSessionManagement() {
	const client = useBackend();
	const cache = useQueryClient();
	const { recheck } = useAccountSession();
	const options = sessionList(client);
	const sessions = useInfiniteQuery(options);
	const [draft, setDraft] = useState<SessionChange | null>(null);
	const update = useMutation({
		mutationKey: sessionChangeKey,
		mutationFn: (change: SessionChange) => changeSession(client, change),
		retry: false,
		networkMode: "always",
		onSettled: async () => {
			// Even a lost response may have rotated or revoked this browser’s session.
			setDraft(null);
			await recheck();
			await cache.invalidateQueries({
				queryKey: options.queryKey,
				exact: true,
			});
		},
	});
	function select(change: SessionChange): void {
		update.reset();
		setDraft(change);
	}
	function confirm(): void {
		if (draft && !update.isPending) update.mutate(draft);
	}
	function retry(): void {
		if (sessions.isFetchNextPageError) void sessions.fetchNextPage();
		else void sessions.refetch();
	}
	return {
		sessions,
		draft,
		update,
		locked: draft !== null || update.isPending,
		select,
		cancel: () => setDraft(null),
		confirm,
		retry,
	};
}
