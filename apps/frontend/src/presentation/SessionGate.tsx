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
		<Group role="status" justify="center" gap="sm" py="xl">
			<Loader size="sm" />
			<Text c="dimmed">{text}</Text>
		</Group>
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
							Open the link sent to <strong>{request.variables}</strong> and
							choose <strong>Confirm sign in</strong>. Your workspace will open
							in the same tab as the link.
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
										{authenticationMessage(request.error)}
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
