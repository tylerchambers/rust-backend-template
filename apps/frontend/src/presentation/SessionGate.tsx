import {
	Alert,
	Button,
	Group,
	Loader,
	Paper,
	Stack,
	Text,
	TextInput,
	Title,
} from "@mantine/core";
import {
	QueryClient,
	QueryClientProvider,
	useIsMutating,
	useMutation,
	useQuery,
	useQueryClient,
} from "@tanstack/react-query";
import {
	createContext,
	type FormEvent,
	type ReactNode,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useState,
} from "react";
import { useBackend } from "../bootstrap/context";
import {
	type Account,
	Authentication,
	AuthenticationError,
	identityQueryKey,
} from "../infrastructure/authentication";
import { sessionChangeKey } from "../infrastructure/sessions";
import { Sessions } from "./Sessions";

function message(error: unknown): string {
	return error instanceof AuthenticationError
		? error.message
		: "The service could not be reached. Check your connection and try again.";
}

function SessionStatus({ text }: { text: string }): React.JSX.Element {
	return (
		<Group role="status" justify="center" gap="sm" py="xl">
			<Loader size="sm" />
			<Text c="dimmed">{text}</Text>
		</Group>
	);
}

type SessionActions = {
	authentication: Authentication;
	checking: boolean;
	recheck: () => Promise<void>;
};

type AccountSession = Pick<SessionActions, "recheck"> & { account: Account };
const AccountSessionContext = createContext<AccountSession | null>(null);

export function useAccountSession(): AccountSession {
	const session = useContext(AccountSessionContext);
	if (!session) throw new Error("Account session is not available");
	return session;
}

function SignIn({
	authentication,
	checking,
	recheck,
}: SessionActions): React.JSX.Element {
	const request = useMutation({
		mutationFn: (email: string) => authentication.requestLink(email),
		retry: false,
		gcTime: 0,
		networkMode: "always",
	});
	function submit(event: FormEvent<HTMLFormElement>): void {
		event.preventDefault();
		const email = new FormData(event.currentTarget).get("email");
		if (!checking && !request.isPending && typeof email === "string") {
			request.mutate(email);
		}
	}

	return (
		<Paper
			withBorder
			shadow="xs"
			radius="lg"
			maw={480}
			mx="auto"
			my="xl"
			p="xl"
		>
			<Stack gap="lg">
				<Stack gap={6}>
					<Text c="teal" size="xs" fw={700} tt="uppercase" lts="0.08em">
						Account access
					</Text>
					<Title order={1} size="h2">
						{request.isSuccess
							? "Check your inbox"
							: "Sign in to your workspace"}
					</Title>
					{request.isSuccess ? (
						<Text c="dimmed" role="status" style={{ overflowWrap: "anywhere" }}>
							Open the link sent to <strong>{request.variables}</strong> in this
							browser and choose <strong>Confirm sign in</strong>. Then return
							here to continue.
						</Text>
					) : (
						<Text c="dimmed">
							Enter your email and we’ll send you a one-time sign-in link. No
							password needed.
						</Text>
					)}
				</Stack>
				{request.isSuccess ? (
					<Stack gap="xs">
						<Button onClick={recheck} loading={checking}>
							Check session
						</Button>
						<Button variant="subtle" onClick={() => request.reset()}>
							Use a different email
						</Button>
					</Stack>
				) : (
					<>
						<form onSubmit={submit}>
							<Stack gap="md">
								<TextInput
									label="Email address"
									placeholder="you@example.com"
									name="email"
									type="email"
									required
									maxLength={254}
									autoComplete="email"
									autoCapitalize="none"
									spellCheck={false}
									disabled={request.isPending}
								/>
								{request.isError && (
									<Alert title="Sign-in link not requested" color="red">
										{message(request.error)}
									</Alert>
								)}
								<Button
									type="submit"
									loading={request.isPending}
									disabled={checking}
								>
									Send sign-in link
								</Button>
							</Stack>
						</form>
						<Group justify="space-between" gap="xs">
							<Text size="sm" c="dimmed">
								Already confirmed your link?
							</Text>
							<Button
								size="xs"
								variant="subtle"
								onClick={recheck}
								loading={checking}
							>
								Check session
							</Button>
						</Group>
					</>
				)}
			</Stack>
		</Paper>
	);
}

function AccountWorkspace({
	account,
	authentication,
	checking,
	recheck,
	children,
}: SessionActions & {
	account: Account;
	children: ReactNode;
}): React.JSX.Element {
	const root = useQueryClient();
	// An old operation can finish after the cookie changes. It may only update its old client.
	const [workspace] = useState(
		() => new QueryClient({ defaultOptions: root.getDefaultOptions() }),
	);
	useEffect(() => () => workspace.clear(), [workspace]);
	const accountSession = useMemo(
		() => ({ account, recheck }),
		[account, recheck],
	);
	const [sessionsOpen, setSessionsOpen] = useState(false);
	const changingSession =
		useIsMutating({ mutationKey: sessionChangeKey }, workspace) > 0;
	const logout = useMutation(
		{
			mutationFn: () => authentication.signOut(),
			onSettled: () =>
				root.invalidateQueries({ queryKey: identityQueryKey, exact: true }),
			networkMode: "always",
			retry: false,
		},
		workspace,
	);

	const verifying = checking || logout.isPending || changingSession;
	return (
		<QueryClientProvider client={workspace}>
			{verifying && (
				<SessionStatus
					text={
						logout.isPending
							? "Signing out…"
							: changingSession
								? "Updating sessions…"
								: "Checking your session…"
					}
				/>
			)}
			<div hidden={verifying}>
				<Stack gap="lg">
					<Paper
						component="section"
						aria-label="Account"
						withBorder
						radius="md"
						px="lg"
						py="sm"
					>
						<Group justify="space-between" gap="sm">
							<Stack gap={0} miw={0}>
								<Text size="xs" c="dimmed">
									Signed in as
								</Text>
								<Text size="sm" fw={600} style={{ overflowWrap: "anywhere" }}>
									{account.email}
								</Text>
							</Stack>
							<Group gap="xs">
								<Button
									variant="subtle"
									size="xs"
									aria-expanded={sessionsOpen}
									aria-controls={sessionsOpen ? "account-sessions" : undefined}
									onClick={() => setSessionsOpen((open) => !open)}
								>
									{sessionsOpen ? "Hide sessions" : "Manage sessions"}
								</Button>
								<Button variant="subtle" size="xs" onClick={recheck}>
									Check session
								</Button>
								<Button
									variant="default"
									size="xs"
									onClick={() => logout.mutate()}
								>
									Sign out
								</Button>
							</Group>
						</Group>
					</Paper>
					{sessionsOpen && <Sessions recheck={recheck} />}
					{logout.isError && (
						<Alert color="red" title="Sign-out not confirmed">
							{message(logout.error)}
						</Alert>
					)}
					<AccountSessionContext.Provider value={accountSession}>
						{children}
					</AccountSessionContext.Provider>
				</Stack>
			</div>
		</QueryClientProvider>
	);
}

export function SessionGate({
	children,
}: {
	children: ReactNode;
}): React.JSX.Element {
	const backend = useBackend();
	const authentication = useMemo(() => new Authentication(backend), [backend]);
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
	if (identity.isPending)
		return <SessionStatus text="Checking your session…" />;
	if (identity.isError) {
		return (
			<Alert title="Session check unavailable" color="orange">
				<Stack align="flex-start" gap="sm">
					<Text size="sm">{message(identity.error)}</Text>
					<Button
						size="xs"
						variant="default"
						loading={identity.isFetching}
						onClick={recheck}
					>
						Try again
					</Button>
				</Stack>
			</Alert>
		);
	}
	if (identity.data === null) {
		return (
			<SignIn
				authentication={authentication}
				checking={identity.isFetching}
				recheck={recheck}
			/>
		);
	}
	return (
		<AccountWorkspace
			key={identity.data.id}
			account={identity.data}
			authentication={authentication}
			checking={identity.isFetching}
			recheck={recheck}
		>
			{children}
		</AccountWorkspace>
	);
}
