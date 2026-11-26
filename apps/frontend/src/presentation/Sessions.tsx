import { Alert, Button, Group, Paper, Stack, Text, Title } from "@mantine/core";
import { useSessionManagement } from "../application/sessionManagement";
import { authenticationMessage } from "../infrastructure/authentication";
import type { SessionChange } from "../infrastructure/sessions";

function details(change: SessionChange): string {
	switch (change.kind) {
		case "rotate":
			return "Replace this browser’s session credential. The old credential will no longer work.";
		case "revokeAll":
			return "Sign out every session for this account, including this browser. You’ll need a new sign-in link.";
		case "revoke":
			return `Revoke session ${change.id}? If it belongs to this browser, you’ll be signed out.`;
	}
}

const connectionError = "Check your connection and try again.";

export function Sessions(): React.JSX.Element {
	const { sessions, draft, update, locked, select, confirm, cancel, retry } =
		useSessionManagement();
	const rows = sessions.data?.pages.flat();
	return (
		<Paper
			id="account-sessions"
			component="section"
			aria-label="Session security"
			withBorder
			p="lg"
		>
			<Stack>
				<Title order={2} size="h3">
					Session security
				</Title>
				<Text c="dimmed" size="sm">
					Sessions are listed by ID. The list does not identify which one
					belongs to this browser.
				</Text>
				<Group>
					<Button
						variant="light"
						disabled={locked}
						onClick={() => select({ kind: "rotate" })}
					>
						Rotate current session
					</Button>
					<Button
						variant="light"
						color="red"
						disabled={locked}
						onClick={() => select({ kind: "revokeAll" })}
					>
						Revoke all sessions
					</Button>
				</Group>
				{draft && (
					<Alert title="Confirm session change" color="orange">
						<Stack align="flex-start" gap="sm">
							<Text size="sm" style={{ overflowWrap: "anywhere" }}>
								{details(draft)}
							</Text>
							<Group>
								<Button
									color={draft.kind === "rotate" ? "teal" : "red"}
									loading={update.isPending}
									onClick={confirm}
								>
									Confirm {draft.kind === "rotate" ? "rotation" : "revocation"}
								</Button>
								<Button
									variant="default"
									disabled={update.isPending}
									onClick={cancel}
								>
									Cancel
								</Button>
							</Group>
						</Stack>
					</Alert>
				)}
				{update.isError && (
					<Alert title="Session change not confirmed" color="red">
						{authenticationMessage(update.error, connectionError)}
					</Alert>
				)}
				{update.isSuccess && (
					<Text role="status">
						{update.variables.kind === "rotate"
							? "Current session rotated."
							: update.variables.kind === "revoke"
								? "Session revoked."
								: "All sessions revoked."}
					</Text>
				)}
				{sessions.isPending && <Text role="status">Loading sessions…</Text>}
				{sessions.isError ? (
					<Alert title="Could not load sessions" color="red">
						<Text size="sm">
							{authenticationMessage(sessions.error, connectionError)}
						</Text>
						<Button
							mt="sm"
							size="xs"
							variant="light"
							loading={sessions.isFetching}
							onClick={retry}
						>
							Retry
						</Button>
					</Alert>
				) : (
					rows && (
						<>
							{rows.length === 0 ? (
								<Text c="dimmed">No active sessions found.</Text>
							) : (
								<Stack component="ul" p={0} m={0} gap="xs">
									{rows.map((id) => (
										<Group component="li" key={id} justify="space-between">
											<Text
												ff="monospace"
												size="sm"
												style={{ overflowWrap: "anywhere" }}
											>
												{id}
											</Text>
											<Button
												size="xs"
												variant="subtle"
												color="red"
												aria-label={`Revoke session ${id}`}
												disabled={locked}
												onClick={() => select({ kind: "revoke", id })}
											>
												Revoke
											</Button>
										</Group>
									))}
								</Stack>
							)}
							{sessions.hasNextPage && (
								<Button
									variant="light"
									style={{ alignSelf: "start" }}
									disabled={sessions.isFetching || locked}
									loading={sessions.isFetchingNextPage}
									onClick={() => void sessions.fetchNextPage()}
								>
									Load more sessions
								</Button>
							)}
						</>
					)
				)}
			</Stack>
		</Paper>
	);
}
