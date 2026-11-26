import {
	Alert,
	Badge,
	Button,
	Group,
	NativeSelect,
	Paper,
	Stack,
	Text,
	TextInput,
	Title,
} from "@mantine/core";
import { useState } from "react";
import { useInvitationManagement } from "../application/teamManagement";
import {
	canRevokeInvitation,
	grantableRoles,
	type InvitationDraft,
	reviewInvitation,
} from "../domain/team";
import { canManageTeam, type WorkspaceRole } from "../domain/workspace";
import {
	isWorkspaceUnavailableToUser,
	workspaceError,
} from "../infrastructure/workspaceResponse";
import { ProofAction } from "./ProofAction";

type Selection =
	| { action: "invite"; draft: InvitationDraft }
	| { action: "revoke_invitation"; id: string };

function ManageInvitations({
	organizationId,
	role,
}: {
	organizationId: string;
	role: WorkspaceRole;
}): React.JSX.Element {
	const { history, invite, revoke } = useInvitationManagement(organizationId);
	const rows = history.data?.pages.flat();
	const [email, setEmail] = useState("");
	const [offeredRole, setOfferedRole] = useState<WorkspaceRole>("viewer");
	const [selection, setSelection] = useState<Selection | null>(null);
	const [notice, setNotice] = useState("");
	const review = reviewInvitation(email, offeredRole, role);
	const roles = grantableRoles(role);
	const selected =
		selection?.action === "revoke_invitation" &&
		!rows?.some(
			(row) => row.id === selection.id && canRevokeInvitation(row.status),
		)
			? null
			: selection;

	return (
		<Stack gap="lg">
			<Title order={2}>Invitations</Title>
			<Text c="dimmed">
				Recipients must sign in with the invited email address, then open their
				invitation email in the same browser and choose{" "}
				<strong>Accept invitation</strong>. Opening the link alone does not join
				the organization.
			</Text>
			{history.isPending && <Text role="status">Loading invitations…</Text>}
			{history.isError && (
				<Alert
					color="red"
					title={
						history.isFetchNextPageError
							? "Could not load more invitations"
							: "Could not load invitations"
					}
				>
					<Text size="sm">{workspaceError(history.error)}</Text>
					<Button
						variant="light"
						size="xs"
						mt="sm"
						loading={history.isFetching}
						onClick={() => {
							if (history.isFetchNextPageError) void history.fetchNextPage();
							else void history.refetch();
						}}
					>
						Retry
					</Button>
				</Alert>
			)}
			{history.isSuccess && (
				<Paper component="section" withBorder p="lg">
					<Stack gap="sm">
						<Title order={3}>Invite a person</Title>
						{selected?.action === "invite" ? (
							<ProofAction
								organizationId={organizationId}
								action="invite"
								target={organizationId}
								label="Send invitation"
								description={`Invite ${selected.draft.email} as ${selected.draft.role}.`}
								submit={(proof) => invite(selected.draft, proof)}
								onCancel={() => setSelection(null)}
								onSuccess={() => {
									setNotice(`Invitation queued for ${selected.draft.email}.`);
									setEmail("");
									setOfferedRole("viewer");
									setSelection(null);
								}}
							/>
						) : (
							<form
								onSubmit={(event) => {
									event.preventDefault();
									if (selected || review.status !== "ready") return;
									setNotice("");
									setSelection({ action: "invite", draft: review.draft });
								}}
							>
								<Stack gap="sm">
									<TextInput
										label="Recipient email"
										type="email"
										required
										maxLength={254}
										autoCapitalize="none"
										spellCheck={false}
										value={email}
										error={
											email && review.status === "invalid-email"
												? "Enter a valid email address."
												: undefined
										}
										onChange={(event) => setEmail(event.currentTarget.value)}
									/>
									<NativeSelect
										label="Invitation role"
										data={roles}
										value={offeredRole}
										onChange={(event) => {
											const next = roles.find(
												(role) => role === event.currentTarget.value,
											);
											if (next) setOfferedRole(next);
										}}
									/>
									<Button
										type="submit"
										style={{ alignSelf: "start" }}
										disabled={
											selected !== null || review.status === "invalid-email"
										}
									>
										Review invitation
									</Button>
								</Stack>
							</form>
						)}
					</Stack>
				</Paper>
			)}
			{rows && !isWorkspaceUnavailableToUser(history.error) && (
				<Stack component="section" aria-label="Invitation history" gap="sm">
					<Title order={3}>Invitation history</Title>
					{notice && <Text role="status">{notice}</Text>}
					{rows.length === 0 && <Text>No invitations yet.</Text>}
					{rows.map((row) => (
						<Paper key={row.id} component="article" withBorder p="md">
							<Stack gap="xs">
								<Group gap="xs">
									<Text fw={600} style={{ overflowWrap: "anywhere" }}>
										{row.email}
									</Text>
									<Badge variant="light">{row.status}</Badge>
								</Group>
								<Text size="sm">
									Role: {row.role} · Expiry:{" "}
									<time dateTime={row.expiresAt}>
										{new Date(row.expiresAt).toLocaleString()}
									</time>
								</Text>
								{history.isSuccess &&
									canRevokeInvitation(row.status) &&
									(selected?.action === "revoke_invitation" &&
									selected.id === row.id ? (
										<ProofAction
											organizationId={organizationId}
											action="revoke_invitation"
											target={row.id}
											label="Revoke invitation"
											description={`Revoke the pending ${row.role} invitation for ${row.email}.`}
											submit={(proof) => revoke(row.id, proof)}
											onCancel={() => setSelection(null)}
											onSuccess={() => {
												setNotice(`Invitation for ${row.email} revoked.`);
												setSelection(null);
											}}
										/>
									) : (
										<Button
											variant="light"
											color="red"
											size="xs"
											style={{ alignSelf: "start" }}
											disabled={selected !== null}
											onClick={() => {
												setNotice("");
												setSelection({
													action: "revoke_invitation",
													id: row.id,
												});
											}}
										>
											Revoke invitation
										</Button>
									))}
							</Stack>
						</Paper>
					))}
					{history.hasNextPage && !history.isFetchNextPageError && (
						<Button
							variant="light"
							style={{ alignSelf: "start" }}
							disabled={history.isFetching}
							loading={history.isFetchingNextPage}
							onClick={() => void history.fetchNextPage()}
						>
							Load more invitations
						</Button>
					)}
				</Stack>
			)}
		</Stack>
	);
}

export function Invitations({
	organizationId,
	role,
}: {
	organizationId: string;
	role: WorkspaceRole;
}): React.JSX.Element {
	if (!canManageTeam(role)) {
		return <Text>Only owners and admins can manage invitations.</Text>;
	}
	return (
		<ManageInvitations
			key={`${organizationId}:${role}`}
			organizationId={organizationId}
			role={role}
		/>
	);
}
