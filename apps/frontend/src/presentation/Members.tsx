import { Alert, Button, Group, NativeSelect, Stack, Text } from "@mantine/core";
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
	return (
		<>
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
						Select role
					</option>
					{roles.map((role) => (
						<option key={role} value={role}>
							{role}
						</option>
					))}
				</NativeSelect>
				<Button
					variant="subtle"
					color="red"
					aria-label={`Remove member ${label}`}
					onClick={() => setDraft({ action: "remove_member" })}
				>
					Remove member
				</Button>
			</Group>
			{draft && (
				<ProofAction
					organizationId={organizationId}
					action={draft.action}
					target={member.userId}
					label={
						draft.action === "change_role" ? "Change role" : "Remove member"
					}
					description={
						draft.action === "change_role"
							? `Change ${label} from ${member.role} to ${draft.role}? This signs them out of all sessions.`
							: `Remove ${label} from this organization? They will lose access to its projects.`
					}
					submit={(proof) => changeMember(draft, proof)}
					onSuccess={() => {
						onSuccess(
							draft.action === "change_role"
								? `Changed the role for ${label} to ${draft.role}.`
								: `Removed ${label}.`,
						);
						setDraft(null);
					}}
					onCancel={() => setDraft(null)}
				/>
			)}
		</>
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
			{notice && !isWorkspaceUnavailableToUser(members.error) && (
				<Text role="status">{notice}</Text>
			)}
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
						variant="default"
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
					{rows.length === 0 ? (
						<Text>No organization members.</Text>
					) : (
						<ul className="record-list" aria-label="Members">
							{rows.map((member) => {
								const access = memberAccess(role, member.role, multipleOwners);
								return (
									<li key={`${member.userId}:${member.role}`}>
										<div className="record-row">
											<div>
												<Text fw={600}>{member.email ?? member.userId}</Text>
												<Text size="sm" c="dimmed">
													{member.role}
													{member.email && ` · ${member.userId}`}
												</Text>
											</div>
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
										</div>
									</li>
								);
							})}
						</ul>
					)}
					{members.hasNextPage && !members.isFetchNextPageError && (
						<Button
							variant="default"
							className="fit-action"
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
