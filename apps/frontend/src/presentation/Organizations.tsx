import {
	Alert,
	Badge,
	Button,
	Group,
	Paper,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useBackend } from "../application/backend";
import { organizationList } from "../infrastructure/organizations";
import {
	isWorkspaceUnavailableToUser,
	workspaceError,
} from "../infrastructure/workspaceResponse";
import { CreateOrganization } from "./CreateOrganization";

export function Organizations(): React.JSX.Element {
	const organizations = useInfiniteQuery(organizationList(useBackend()));
	const rows = organizations.data?.pages.flat();
	return (
		<Stack gap="xl">
			<header>
				<Title order={1}>Organizations</Title>
				<Text c="dimmed">
					Choose an organization to see its projects and members.
				</Text>
			</header>
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
							size="xs"
							variant="light"
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
						{rows.length === 0 ? (
							<Paper withBorder p="lg">
								<Title order={2} size="h4">
									No organizations yet
								</Title>
								<Text c="dimmed" size="sm">
									Create your first organization below, or ask a team owner for
									an invitation.
								</Text>
							</Paper>
						) : (
							<Stack
								component="ul"
								p={0}
								m={0}
								gap="xs"
								style={{ listStyle: "none" }}
							>
								{rows.map((org) => (
									<Paper key={org.id} component="li" withBorder p="md">
										<Group justify="space-between">
											<Link
												to="/organizations/$organizationId"
												params={{ organizationId: org.id }}
												style={{ overflowWrap: "anywhere" }}
											>
												{org.name}
											</Link>
											<Badge variant="light">{org.role}</Badge>
										</Group>
									</Paper>
								))}
							</Stack>
						)}
						{organizations.hasNextPage &&
							!organizations.isFetchNextPageError && (
								<Button
									variant="light"
									style={{ alignSelf: "start" }}
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
			<CreateOrganization />
		</Stack>
	);
}
