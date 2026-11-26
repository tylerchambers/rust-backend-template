import { Alert, Button, Paper, Stack, Text, Title } from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useBackend } from "../application/backend";
import { auditEvents } from "../infrastructure/audit";
import { workspaceError } from "../infrastructure/workspaceResponse";

export function AuditEvents({
	organizationId,
}: {
	organizationId: string;
}): React.JSX.Element {
	const events = useInfiniteQuery(auditEvents(useBackend(), organizationId));
	const rows = events.data?.pages.flat();
	return (
		<Stack component="section" aria-busy={events.isFetching}>
			<Title order={2}>Organization audit events</Title>
			{events.isPending && <Text role="status">Loading events…</Text>}
			{events.isError ? (
				<Alert color="red" title="Could not load audit events">
					<Text size="sm">{workspaceError(events.error)}</Text>
					<Button
						mt="sm"
						size="xs"
						variant="light"
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
			) : (
				rows && (
					<>
						{rows.length === 0 ? (
							<Text>No events recorded yet.</Text>
						) : (
							<>
								<Text size="sm" c="dimmed">
									Events are listed by ID, not by time.
								</Text>
								<Stack
									component="ul"
									p={0}
									m={0}
									gap="xs"
									style={{ listStyle: "none" }}
								>
									{rows.map((event) => (
										<Paper
											key={event.id}
											component="li"
											withBorder
											p="md"
											style={{ overflowWrap: "anywhere" }}
										>
											<Text fw={600}>{event.action}</Text>
											<Text size="sm" c="dimmed">
												<time dateTime={event.occurredAt}>
													{new Date(event.occurredAt).toLocaleString()}
												</time>
											</Text>
											<Text size="sm">Actor: {event.actorId}</Text>
											<Text size="sm">Subject: {event.subject}</Text>
										</Paper>
									))}
								</Stack>
							</>
						)}
						{events.hasNextPage && (
							<Button
								variant="light"
								style={{ alignSelf: "start" }}
								loading={events.isFetchingNextPage}
								disabled={events.isFetching}
								onClick={() => void events.fetchNextPage()}
							>
								Load more events
							</Button>
						)}
					</>
				)
			)}
		</Stack>
	);
}
