import {
	QueryClient,
	QueryClientProvider,
	useIsMutating,
	useMutation,
	useQueryClient,
} from "@tanstack/react-query";
import {
	createContext,
	type ReactNode,
	useContext,
	useEffect,
	useMemo,
	useState,
} from "react";
import {
	type Account,
	identityQueryKey,
} from "../infrastructure/authentication";
import { sessionChangeKey } from "../infrastructure/sessions";
import { useAuthentication } from "./backend";

type AccountSession = {
	account: Account;
	recheck: () => Promise<void>;
	invalidateIdentity: () => Promise<void>;
};
const AccountSessionContext = createContext<AccountSession | null>(null);

export function useAccountSession(): AccountSession {
	const session = useContext(AccountSessionContext);
	if (!session) throw new Error("Account session is not available");
	return session;
}

function AccountCache({
	children,
}: {
	children: ReactNode;
}): React.JSX.Element {
	const root = useQueryClient();
	// An old operation can finish after the cookie changes. It may only update its old client.
	const [workspace] = useState(
		() => new QueryClient({ defaultOptions: root.getDefaultOptions() }),
	);
	useEffect(() => () => workspace.clear(), [workspace]);
	return (
		<QueryClientProvider client={workspace}>{children}</QueryClientProvider>
	);
}

export function AccountSessionProvider({
	account,
	recheck,
	children,
}: Pick<AccountSession, "account" | "recheck"> & {
	children: ReactNode;
}): React.JSX.Element {
	const root = useQueryClient();
	const session = useMemo(
		() => ({
			account,
			recheck,
			invalidateIdentity: () =>
				root.invalidateQueries({ queryKey: identityQueryKey, exact: true }),
		}),
		[account, recheck, root],
	);
	return (
		<AccountSessionContext.Provider value={session}>
			<AccountCache key={account.id}>{children}</AccountCache>
		</AccountSessionContext.Provider>
	);
}

export function useAccountActivity() {
	const authentication = useAuthentication();
	const { invalidateIdentity } = useAccountSession();
	const changingSession = useIsMutating({ mutationKey: sessionChangeKey }) > 0;
	const logout = useMutation({
		mutationFn: () => authentication.signOut(),
		onSettled: invalidateIdentity,
		networkMode: "always",
		retry: false,
	});
	return { changingSession, logout };
}
