import { Alert, Button, Group, Stack, Text, Title } from "@mantine/core";
import { useSessionManagement } from "../application/sessionManagement";
import { authenticationMessage } from "../infrastructure/authentication";
import type { SessionChange } from "../infrastructure/sessions";

function details(change: SessionChange): string {
	switch (change.kind) {
		case "rotate":
			return "The old credential for this browser will stop working.";
		case "revokeAll":
			return "This signs out every browser, including this one. You'll need a new sign-in link.";
		case "revoke":
			return `Revoke session ${change.id}? If this is your session, you'll be signed out.`;
	}
}

const connectionError = "Check your connection and try again.";

export function Sessions(): React.JSX.Element {
	const { sessions, draft, update, locked, select, confirm, cancel, retry } =
		useSessionManagement();
	const rows = sessions.data?.pages.flat();
	return (
		<Stack component="section" aria-labelledby="sessions-title" gap="sm">
			<Title order={2} id="sessions-title">
				Sessions
			</Title>
			<Group gap="sm">
				<Button
					variant="default"
					disabled={locked}
					onClick={() => select({ kind: "rotate" })}
				>
					Rotate current session
				</Button>
				<Button
					variant="subtle"
					color="red"
					disabled={locked}
					onClick={() => select({ kind: "revokeAll" })}
				>
					Sign out everywhere
				</Button>
			</Group>
			{draft && (
				<Stack gap="sm" role="group" aria-label="Confirm session change">
					<Text>{details(draft)}</Text>
					<Group gap="sm">
						<Button
							{...(draft.kind === "rotate"
								? { variant: "filled" }
								: { variant: "subtle", color: "red" })}
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
			{sessions.isError && (
				<Alert title="Could not load sessions" color="red">
					<Stack gap="sm" align="flex-start">
						<Text size="sm">
							{authenticationMessage(sessions.error, connectionError)}
						</Text>
						<Button
							variant="default"
							loading={sessions.isFetching}
							onClick={retry}
						>
							Retry
						</Button>
					</Stack>
				</Alert>
			)}
			{!sessions.isError && rows && (
				<>
					{rows.length === 0 ? (
						<Text c="dimmed">No active sessions.</Text>
					) : (
						<>
							<Text c="dimmed">
								The list does not identify this browser’s session.
							</Text>
							<ul className="record-list" aria-label="Active sessions">
								{rows.map((id) => (
									<li key={id}>
										<div className="record-row">
											<Text>{id}</Text>
											<Button
												variant="subtle"
												color="red"
												aria-label={`Revoke session ${id}`}
												disabled={locked}
												onClick={() => select({ kind: "revoke", id })}
											>
												Revoke
											</Button>
										</div>
									</li>
								))}
							</ul>
						</>
					)}
					{sessions.hasNextPage && (
						<Button
							className="fit-action"
							variant="default"
							disabled={sessions.isFetching || locked}
							loading={sessions.isFetchingNextPage}
							onClick={() => void sessions.fetchNextPage()}
						>
							Load more sessions
						</Button>
					)}
				</>
			)}
		</Stack>
	);
}
