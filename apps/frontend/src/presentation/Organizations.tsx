import { Alert, Button, Group, Stack, Table, Text, Title } from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useBackend } from "../application/backend";
import { roleLabel } from "../domain/workspace";
import { organizationList } from "../infrastructure/organizations";
import {
	isWorkspaceUnavailableToUser,
	workspaceError,
} from "../infrastructure/workspaceResponse";

export function Organizations(): React.JSX.Element {
	const organizations = useInfiniteQuery(organizationList(useBackend()));
	const rows = organizations.data?.pages.flat();
	return (
		<Stack gap="lg">
			<Group justify="space-between">
				<Title order={1}>Organizations</Title>
				<Button
					ml="auto"
					renderRoot={(props) => <Link to="/organizations/new" {...props} />}
				>
					New organization
				</Button>
			</Group>
			<section
				aria-label="Your organizations"
				aria-busy={organizations.isFetching}
			>
				{organizations.isPending && (
					<Text role="status">Loading organizations…</Text>
				)}
				{organizations.isError && (
					<Alert
						color="red"
						title={
							organizations.isFetchNextPageError
								? "Could not load more organizations"
								: "Could not load organizations"
						}
					>
						<Text size="sm">{workspaceError(organizations.error)}</Text>
						<Button
							mt="sm"
							variant="default"
							loading={organizations.isFetching}
							onClick={() => {
								if (organizations.isFetchNextPageError)
									void organizations.fetchNextPage();
								else void organizations.refetch();
							}}
						>
							Retry
						</Button>
					</Alert>
				)}
				{rows && !isWorkspaceUnavailableToUser(organizations.error) && (
					<Stack>
						<Table aria-label="Active organizations">
							<Table.Thead>
								<Table.Tr>
									<Table.Th scope="col">Organization</Table.Th>
									<Table.Th scope="col">Your role</Table.Th>
								</Table.Tr>
							</Table.Thead>
							<Table.Tbody>
								{rows.length === 0 && (
									<Table.Tr>
										<Table.Td colSpan={2}>No organizations yet.</Table.Td>
									</Table.Tr>
								)}
								{rows.map((org) => (
									<Table.Tr key={org.id}>
										<Table.Td>
											<Link
												to="/organizations/$organizationId"
												params={{ organizationId: org.id }}
											>
												{org.name}
											</Link>
										</Table.Td>
										<Table.Td>{roleLabel(org.isAdmin)}</Table.Td>
									</Table.Tr>
								))}
							</Table.Tbody>
						</Table>
						{organizations.hasNextPage &&
							!organizations.isFetchNextPageError && (
								<Button
									variant="default"
									className="fit-action"
									disabled={organizations.isFetching}
									loading={organizations.isFetchingNextPage}
									onClick={() => void organizations.fetchNextPage()}
								>
									Load more organizations
								</Button>
							)}
					</Stack>
				)}
			</section>
		</Stack>
	);
}
