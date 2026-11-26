import { Alert, Button, Stack, Table, Text, Title } from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useBackend } from "../application/backend";
import { useInvitationManagement } from "../application/teamManagement";
import { canRevokeInvitation } from "../domain/team";
import { roleLabel } from "../domain/workspace";
import { invitationList } from "../infrastructure/invitations";
import {
	isWorkspaceUnavailableToUser,
	workspaceError,
} from "../infrastructure/workspaceResponse";
import { ConfirmAction } from "./ConfirmAction";

export function Invitations({
	organizationId,
}: {
	organizationId: string;
}): React.JSX.Element {
	const history = useInfiniteQuery(
		invitationList(useBackend(), organizationId),
	);
	const { revoke } = useInvitationManagement(organizationId);
	const rows = history.data?.pages.flat();
	const [selectedId, setSelectedId] = useState<string | null>(null);
	const [notice, setNotice] = useState("");
	const selected = rows?.find(
		(row) => row.id === selectedId && canRevokeInvitation(row.status),
	);

	return (
		<Stack component="section" gap="sm" aria-label="Invitation history">
			<Title order={2}>Invitations</Title>
			{notice && !isWorkspaceUnavailableToUser(history.error) && (
				<Text role="status">{notice}</Text>
			)}
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
						variant="default"
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
			{rows && !isWorkspaceUnavailableToUser(history.error) && (
				<>
					<Table.ScrollContainer type="native" minWidth={600}>
						<Table aria-label="Invitations">
							<Table.Thead>
								<Table.Tr>
									<Table.Th scope="col">Recipient</Table.Th>
									<Table.Th scope="col">Role</Table.Th>
									<Table.Th scope="col">Status</Table.Th>
									<Table.Th scope="col">Expires</Table.Th>
									<Table.Th scope="col">Actions</Table.Th>
								</Table.Tr>
							</Table.Thead>
							<Table.Tbody>
								{rows.length === 0 && (
									<Table.Tr>
										<Table.Td colSpan={5}>No invitations yet.</Table.Td>
									</Table.Tr>
								)}
								{rows.map((row) => (
									<Table.Tr key={row.id}>
										<Table.Td>{row.email}</Table.Td>
										<Table.Td>{roleLabel(row.isAdmin)}</Table.Td>
										<Table.Td>{row.status}</Table.Td>
										<Table.Td>
											<time dateTime={row.expiresAt}>
												{new Date(row.expiresAt).toLocaleString()}
											</time>
										</Table.Td>
										<Table.Td>
											{history.isSuccess && canRevokeInvitation(row.status) && (
												<Button
													variant="subtle"
													color="red"
													aria-label={`Revoke invitation for ${row.email}`}
													onClick={() => {
														setNotice("");
														setSelectedId(row.id);
													}}
												>
													Revoke
												</Button>
											)}
										</Table.Td>
									</Table.Tr>
								))}
							</Table.Tbody>
						</Table>
					</Table.ScrollContainer>
					{history.isSuccess && selected && (
						<ConfirmAction
							key={selected.id}
							destructive
							label="Revoke invitation"
							description={`Revoke the ${roleLabel(selected.isAdmin)} invitation for ${selected.email}?`}
							submit={() => revoke(selected.id)}
							onCancel={() => setSelectedId(null)}
							onSuccess={() => {
								setNotice(`Invitation for ${selected.email} revoked.`);
								setSelectedId(null);
							}}
						/>
					)}
					{history.hasNextPage && !history.isFetchNextPageError && (
						<Button
							variant="default"
							className="fit-action"
							disabled={history.isFetching}
							loading={history.isFetchingNextPage}
							onClick={() => void history.fetchNextPage()}
						>
							Load more invitations
						</Button>
					)}
				</>
			)}
		</Stack>
	);
}
