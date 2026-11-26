import {
	Alert,
	Button,
	Group,
	Stack,
	Text,
	TextInput,
	Title,
} from "@mantine/core";
import { type FormEvent, type ReactNode, useState } from "react";
import {
	AccountSessionProvider,
	useAccountActivity,
	useAccountSession,
} from "../application/accountSession";
import { useIdentity, useSignInLink } from "../application/authentication";
import { authenticationMessage } from "../infrastructure/authentication";
import { Sessions } from "./Sessions";

function SessionStatus({ text }: { text: string }): React.JSX.Element {
	return (
		<Text role="status" c="dimmed">
			{text}
		</Text>
	);
}

type SessionCheck = {
	checking: boolean;
	recheck: () => Promise<void>;
};

function SignIn({ checking, recheck }: SessionCheck): React.JSX.Element {
	const { request, requestLink } = useSignInLink(checking);
	function submit(event: FormEvent<HTMLFormElement>): void {
		event.preventDefault();
		const email = new FormData(event.currentTarget).get("email");
		if (typeof email === "string") requestLink(email);
	}

	return (
		<Stack className="entry-form" gap="lg">
			<Title order={1}>
				{request.isSuccess ? "Check your inbox" : "Sign in"}
			</Title>
			{request.isSuccess ? (
				<Stack gap="sm">
					<Text role="status">
						Open the sign-in link sent to <strong>{request.variables}</strong>.
					</Text>
					<Group gap="sm">
						<Button onClick={recheck} loading={checking}>
							Check session
						</Button>
						<Button variant="default" onClick={() => request.reset()}>
							Use a different email
						</Button>
					</Group>
				</Stack>
			) : (
				<form onSubmit={submit}>
					<Stack gap="sm">
						<TextInput
							label="Email address"
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
							<Alert title="Could not request a sign-in link" color="red">
								{authenticationMessage(request.error)}
							</Alert>
						)}
						<Group gap="sm">
							<Button
								type="submit"
								loading={request.isPending}
								disabled={checking}
							>
								Send sign-in link
							</Button>
							<Button
								type="button"
								variant="default"
								onClick={recheck}
								loading={checking}
							>
								Check session
							</Button>
						</Group>
					</Stack>
				</form>
			)}
		</Stack>
	);
}

function AccountWorkspace({
	checking,
	children,
}: {
	checking: boolean;
	children: ReactNode;
}): React.JSX.Element {
	const { account, recheck } = useAccountSession();
	const { changingSession, logout } = useAccountActivity();
	const [sessionsOpen, setSessionsOpen] = useState(false);

	const verifying = checking || logout.isPending || changingSession;
	return (
		<>
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
					<section aria-label="Account">
						<div className="record-row">
							<Text>{account.email}</Text>
							<Group gap="sm">
								<Button
									variant="default"
									aria-expanded={sessionsOpen}
									aria-controls={sessionsOpen ? "account-sessions" : undefined}
									onClick={() => setSessionsOpen((open) => !open)}
								>
									{sessionsOpen ? "Hide sessions" : "Manage sessions"}
								</Button>
								<Button variant="default" onClick={recheck}>
									Check session
								</Button>
								<Button variant="default" onClick={() => logout.mutate()}>
									Sign out
								</Button>
							</Group>
						</div>
					</section>
					{sessionsOpen && <Sessions />}
					{logout.isError && (
						<Alert color="red" title="Sign-out not confirmed">
							{authenticationMessage(logout.error)}
						</Alert>
					)}
					{children}
				</Stack>
			</div>
		</>
	);
}

export function SessionGate({
	children,
}: {
	children: ReactNode;
}): React.JSX.Element {
	const { identity, recheck } = useIdentity();
	if (identity.isPending)
		return <SessionStatus text="Checking your session…" />;
	if (identity.isError) {
		return (
			<Alert title="Session check unavailable" color="orange">
				<Stack align="flex-start" gap="sm">
					<Text size="sm">{authenticationMessage(identity.error)}</Text>
					<Button
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
		return <SignIn checking={identity.isFetching} recheck={recheck} />;
	}
	return (
		<AccountSessionProvider account={identity.data} recheck={recheck}>
			<AccountWorkspace checking={identity.isFetching}>
				{children}
			</AccountWorkspace>
		</AccountSessionProvider>
	);
}
