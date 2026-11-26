import {
	Alert,
	Badge,
	Button,
	Group,
	NativeSelect,
	Paper,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useBackend } from "../application/backend";
import {
	type MemberChange,
	useChangeMember,
} from "../application/teamManagement";
import {
	grantableRoles,
	hasMultipleOwners,
	type Member,
	memberAccess,
} from "../domain/team";
import type { WorkspaceRole } from "../domain/workspace";
import { memberList } from "../infrastructure/members";
import {
	isWorkspaceUnavailableToUser,
	workspaceError,
} from "../infrastructure/workspaceResponse";
import { ProofAction } from "./ProofAction";

type Props = { organizationId: string; role: WorkspaceRole };

function MemberActions({
	organizationId,
	role,
	member,
	onSuccess,
}: Props & {
	member: Member;
	onSuccess: (notice: string) => void;
}): React.JSX.Element {
	const changeMember = useChangeMember(organizationId, member.userId);
	const [draft, setDraft] = useState<MemberChange | null>(null);
	const label = member.email ?? member.userId;
	const roles = grantableRoles(role, member.role);
	if (!draft) {
		return (
			<Group align="end" gap="sm">
				<NativeSelect
					label="Change role"
					aria-label={`Change role for ${label}`}
					value=""
					onChange={(event) => {
						const next = roles.find(
							(role) => role === event.currentTarget.value,
						);
						if (next) setDraft({ action: "change_role", role: next });
					}}
				>
					<option value="" disabled>
						Select a new role
					</option>
					{roles.map((role) => (
						<option key={role} value={role}>
							{role}
						</option>
					))}
				</NativeSelect>
				<Button
					variant="light"
					color="red"
					aria-label={`Remove ${label}`}
					onClick={() => setDraft({ action: "remove_member" })}
				>
					Remove member
				</Button>
			</Group>
		);
	}
	const change = draft.action === "change_role";
	return (
		<ProofAction
			organizationId={organizationId}
			action={draft.action}
			target={member.userId}
			label={change ? "Change role" : "Remove member"}
			description={
				change
					? `Change ${label} from ${member.role} to ${draft.role}? This signs them out of all sessions.`
					: `Remove ${label} from this organization? They will lose access to its projects.`
			}
			submit={(proof) => changeMember(draft, proof)}
			onSuccess={() => {
				onSuccess(
					change
						? `Changed the role for ${label} to ${draft.role}.`
						: `Removed ${label}.`,
				);
				setDraft(null);
			}}
			onCancel={() => setDraft(null)}
		/>
	);
}

function OrganizationMembers({
	organizationId,
	role,
}: Props): React.JSX.Element {
	const client = useBackend();
	const members = useInfiniteQuery(memberList(client, organizationId, role));
	const [notice, setNotice] = useState("");
	const rows = members.data?.pages.flat();
	const multipleOwners = rows !== undefined && hasMultipleOwners(rows);
	return (
		<Stack
			component="section"
			aria-label="Organization members"
			aria-busy={members.isFetching}
		>
			<Title order={2}>Members</Title>
			{members.isPending && <Text role="status">Loading members…</Text>}
			{members.isError && (
				<Alert
					title={
						members.isFetchNextPageError
							? "Could not load more members"
							: "Could not load members"
					}
					color="red"
				>
					<Text size="sm">{workspaceError(members.error)}</Text>
					<Button
						variant="light"
						size="xs"
						mt="sm"
						loading={members.isFetching}
						onClick={() => {
							if (members.isFetchNextPageError) void members.fetchNextPage();
							else void members.refetch();
						}}
					>
						Retry
					</Button>
				</Alert>
			)}
			{rows && !isWorkspaceUnavailableToUser(members.error) && (
				<>
					{notice && <Text role="status">{notice}</Text>}
					{rows.length === 0 ? (
						<Text>No organization members.</Text>
					) : (
						<Stack
							component="ul"
							m={0}
							p={0}
							gap="xs"
							style={{ listStyle: "none" }}
						>
							{rows.map((member) => {
								const access = memberAccess(role, member.role, multipleOwners);
								return (
									<Paper
										key={`${member.userId}:${member.role}`}
										component="li"
										withBorder
										p="md"
									>
										<Stack gap="xs">
											<Group>
												<Text fw={600} style={{ overflowWrap: "anywhere" }}>
													{member.email ?? member.userId}
												</Text>
												<Badge variant="light">{member.role}</Badge>
											</Group>
											{member.email && (
												<Text
													size="sm"
													c="dimmed"
													style={{ overflowWrap: "anywhere" }}
												>
													{member.userId}
												</Text>
											)}
											{members.isSuccess && access === "editable" && (
												<MemberActions
													organizationId={organizationId}
													role={role}
													member={member}
													onSuccess={setNotice}
												/>
											)}
											{access === "last-owner" && (
												<Text size="sm" c="dimmed">
													{members.hasNextPage
														? "Load more members to check for another owner."
														: "The organization must keep at least one owner."}
												</Text>
											)}
										</Stack>
									</Paper>
								);
							})}
						</Stack>
					)}
					{members.hasNextPage && !members.isFetchNextPageError && (
						<Button
							variant="light"
							style={{ alignSelf: "start" }}
							disabled={members.isFetching}
							loading={members.isFetchingNextPage}
							onClick={() => void members.fetchNextPage()}
						>
							Load more members
						</Button>
					)}
				</>
			)}
		</Stack>
	);
}

export function Members({ organizationId, role }: Props): React.JSX.Element {
	return (
		<OrganizationMembers
			key={`${organizationId}:${role}`}
			organizationId={organizationId}
			role={role}
		/>
	);
}
