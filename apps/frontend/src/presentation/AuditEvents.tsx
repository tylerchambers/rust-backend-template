import { Alert, Button, Stack, Text } from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useBackend } from "../application/backend";
import { auditEvents } from "../infrastructure/audit";
import {
	isWorkspaceUnavailableToUser,
	workspaceError,
} from "../infrastructure/workspaceResponse";

export function AuditEvents({
	organizationId,
}: {
	organizationId: string;
}): React.JSX.Element {
	const events = useInfiniteQuery(auditEvents(useBackend(), organizationId));
	const rows = events.data?.pages.flat();
	return (
		<Stack
			component="section"
			aria-label="Audit events"
			aria-busy={events.isFetching}
		>
			{events.isPending && <Text role="status">Loading events…</Text>}
			{events.isError && (
				<Alert
					color="red"
					title={
						events.isFetchNextPageError
							? "Could not load more events"
							: "Could not load audit events"
					}
				>
					<Text size="sm">{workspaceError(events.error)}</Text>
					<Button
						mt="sm"
						variant="default"
						loading={events.isFetching}
						onClick={() =>
							void (events.isFetchNextPageError
								? events.fetchNextPage()
								: events.refetch())
						}
					>
						Retry
					</Button>
				</Alert>
			)}
			{rows && !isWorkspaceUnavailableToUser(events.error) && (
				<>
					{rows.length === 0 ? (
						<Text>No events recorded yet.</Text>
					) : (
						<>
							<Text size="sm" c="dimmed">
								Newest first
							</Text>
							<ol className="record-list" aria-label="Events">
								{rows.map((event) => (
									<li key={event.id}>
										<div className="record-row">
											<Text fw={600}>{event.action}</Text>
											<Text size="sm" c="dimmed">
												<time dateTime={event.occurredAt}>
													{new Date(event.occurredAt).toLocaleString()}
												</time>
											</Text>
										</div>
										<Text size="sm">Actor: {event.actorId}</Text>
										<Text size="sm">Subject: {event.subject}</Text>
									</li>
								))}
							</ol>
						</>
					)}
					{events.hasNextPage && !events.isFetchNextPageError && (
						<Button
							variant="default"
							className="fit-action"
							loading={events.isFetchingNextPage}
							disabled={events.isFetching}
							onClick={() => void events.fetchNextPage()}
						>
							Load more events
						</Button>
					)}
				</>
			)}
		</Stack>
	);
}
