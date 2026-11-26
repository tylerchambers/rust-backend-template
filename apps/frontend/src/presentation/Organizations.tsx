import { Alert, Button, Stack, Text, Title } from "@mantine/core";
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
		<Stack gap="lg">
			<Title order={1}>Organizations</Title>
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
						{rows.length === 0 ? (
							<Text c="dimmed">No organizations yet.</Text>
						) : (
							<ul className="record-list">
								{rows.map((org) => (
									<li key={org.id}>
										<div className="record-row">
											<Link
												to="/organizations/$organizationId"
												params={{ organizationId: org.id }}
											>
												{org.name}
											</Link>
											<Text c="dimmed">{org.role}</Text>
										</div>
									</li>
								))}
							</ul>
						)}
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
			<CreateOrganization />
		</Stack>
	);
}
