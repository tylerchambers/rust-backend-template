import {
	Alert,
	Button,
	Group,
	NativeSelect,
	Stack,
	Table,
	Text,
} from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useBackend } from "../application/backend";
import {
	type MemberChange,
	useChangeMember,
} from "../application/teamManagement";
import { hasMultipleAdmins, type Member } from "../domain/team";
import { roleLabel } from "../domain/workspace";
import { memberList } from "../infrastructure/members";
import {
	isWorkspaceUnavailableToUser,
	workspaceError,
} from "../infrastructure/workspaceResponse";
import { ConfirmAction } from "./ConfirmAction";

type Props = { organizationId: string; isAdmin: boolean };

function MemberActions({
	organizationId,
	member,
	onSuccess,
}: {
	organizationId: string;
	member: Member;
	onSuccess: (notice: string) => void;
}): React.JSX.Element {
	const changeMember = useChangeMember(organizationId, member.userId);
	const [draft, setDraft] = useState<MemberChange | null>(null);
	const label = member.email ?? member.userId;
	const nextIsAdmin = !member.isAdmin;
	return (
		<>
			<Group align="end" gap="sm">
				<NativeSelect
					label="Change role"
					aria-label={`Change role for ${label}`}
					value=""
					onChange={() =>
						setDraft({ action: "change_role", isAdmin: nextIsAdmin })
					}
				>
					<option value="" disabled>
						Select role
					</option>
					<option value={nextIsAdmin ? "admin" : "member"}>
						{roleLabel(nextIsAdmin)}
					</option>
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
				<ConfirmAction
					destructive={draft.action === "remove_member"}
					label={
						draft.action === "change_role" ? "Change role" : "Remove member"
					}
					description={
						draft.action === "change_role"
							? `Change ${label} from ${roleLabel(member.isAdmin)} to ${roleLabel(draft.isAdmin)}? This signs them out of all sessions.`
							: `Remove ${label} from this organization? They will lose access to its projects.`
					}
					submit={() => changeMember(draft)}
					onSuccess={() => {
						onSuccess(
							draft.action === "change_role"
								? `Changed the role for ${label} to ${roleLabel(draft.isAdmin)}.`
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
	isAdmin,
}: Props): React.JSX.Element {
	const client = useBackend();
	const members = useInfiniteQuery(memberList(client, organizationId, isAdmin));
	const [notice, setNotice] = useState("");
	const rows = members.data?.pages.flat();
	const multipleAdmins = rows !== undefined && hasMultipleAdmins(rows);
	return (
		<Stack
			component="section"
			miw={0}
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
					<Table.ScrollContainer minWidth={isAdmin ? 540 : 0} type="native">
						<Table aria-label="Members">
							<Table.Thead>
								<Table.Tr>
									<Table.Th scope="col">Member</Table.Th>
									<Table.Th scope="col">Role</Table.Th>
									{isAdmin && <Table.Th scope="col">Actions</Table.Th>}
								</Table.Tr>
							</Table.Thead>
							<Table.Tbody>
								{rows.length === 0 && (
									<Table.Tr>
										<Table.Td colSpan={isAdmin ? 3 : 2}>
											No organization members.
										</Table.Td>
									</Table.Tr>
								)}
								{rows.map((member) => {
									const protectedAdmin = member.isAdmin && !multipleAdmins;
									return (
										<Table.Tr key={`${member.userId}:${member.isAdmin}`}>
											<Table.Td>
												<Text fw={600}>{member.email ?? member.userId}</Text>
												{member.email && (
													<Text size="sm" c="dimmed">
														{member.userId}
													</Text>
												)}
											</Table.Td>
											<Table.Td>{roleLabel(member.isAdmin)}</Table.Td>
											{isAdmin && (
												<Table.Td>
													{members.isSuccess && !protectedAdmin && (
														<MemberActions
															organizationId={organizationId}
															member={member}
															onSuccess={setNotice}
														/>
													)}
													{protectedAdmin && (
														<Text size="sm" c="dimmed">
															{members.hasNextPage
																? "Load more members to check for another admin."
																: "The organization must keep at least one admin."}
														</Text>
													)}
												</Table.Td>
											)}
										</Table.Tr>
									);
								})}
							</Table.Tbody>
						</Table>
					</Table.ScrollContainer>
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

export function Members({ organizationId, isAdmin }: Props): React.JSX.Element {
	return (
		<OrganizationMembers
			key={`${organizationId}:${isAdmin}`}
			organizationId={organizationId}
			isAdmin={isAdmin}
		/>
	);
}
