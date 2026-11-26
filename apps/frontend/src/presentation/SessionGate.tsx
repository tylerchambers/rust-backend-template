import {
	Alert,
	Avatar,
	Button,
	Menu,
	Stack,
	Text,
	TextInput,
	Title,
	UnstyledButton,
} from "@mantine/core";
import { Link } from "@tanstack/react-router";
import type { FormEvent, ReactNode } from "react";
import {
	AccountSessionProvider,
	useAccountActivity,
	useAccountSession,
} from "../application/accountSession";
import { useIdentity, useSignInLink } from "../application/authentication";
import { authenticationMessage } from "../infrastructure/authentication";
import { ProjectDesk } from "./ProjectDesk";

function SessionStatus({ text }: { text: string }): React.JSX.Element {
	return (
		<Text role="status" c="dimmed">
			{text}
		</Text>
	);
}

function SignIn({ checking }: { checking: boolean }): React.JSX.Element {
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
					<Button
						className="fit-action"
						variant="default"
						onClick={() => request.reset()}
					>
						Use a different email
					</Button>
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
						<Button
							className="fit-action"
							type="submit"
							loading={request.isPending}
							disabled={checking}
						>
							Send sign-in link
						</Button>
					</Stack>
				</form>
			)}
		</Stack>
	);
}

function ProfileMenu({
	email,
	onLogout,
}: {
	email: string;
	onLogout: () => void;
}): React.JSX.Element {
	return (
		<Menu position="bottom-end">
			<Menu.Target>
				<UnstyledButton
					className="profile-button"
					aria-label={`Profile: ${email}`}
				>
					<Avatar size="sm" radius="xl" aria-hidden="true" />
					<Text component="span" truncate>
						{email}
					</Text>
				</UnstyledButton>
			</Menu.Target>
			<Menu.Dropdown>
				<Menu.Item
					renderRoot={(props) => <Link to="/account/settings" {...props} />}
				>
					Account settings
				</Menu.Item>
				<Menu.Divider />
				<Menu.Item onClick={onLogout}>Log out</Menu.Item>
			</Menu.Dropdown>
		</Menu>
	);
}

function AccountWorkspace({
	checking,
	children,
}: {
	checking: boolean;
	children: ReactNode;
}): React.JSX.Element {
	const { account } = useAccountSession();
	const { changingSession, logout } = useAccountActivity();

	const verifying = checking || logout.isPending || changingSession;
	return (
		<ProjectDesk
			profile={
				!verifying && (
					<ProfileMenu email={account.email} onLogout={() => logout.mutate()} />
				)
			}
		>
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
					{logout.isError && (
						<Alert color="red" title="Sign-out not confirmed">
							{authenticationMessage(logout.error)}
						</Alert>
					)}
					{children}
				</Stack>
			</div>
		</ProjectDesk>
	);
}

export function SessionGate({
	children,
}: {
	children: ReactNode;
}): React.JSX.Element {
	const { identity, recheck } = useIdentity();
	let content: ReactNode;
	if (identity.isPending) {
		content = <SessionStatus text="Checking your session…" />;
	} else if (identity.isError) {
		content = (
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
	} else if (identity.data === null) {
		content = <SignIn checking={identity.isFetching} />;
	} else {
		return (
			<AccountSessionProvider account={identity.data} recheck={recheck}>
				<AccountWorkspace checking={identity.isFetching}>
					{children}
				</AccountWorkspace>
			</AccountSessionProvider>
		);
	}
	return <ProjectDesk>{content}</ProjectDesk>;
}
