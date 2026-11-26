import { infiniteQueryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { nextIdCursor, workspacePageSize } from "./workspacePage";
import { workspaceBody } from "./workspaceResponse";

const events = z
	.array(
		z
			.object({
				id: z.uuid(),
				actor_id: z.uuid(),
				action: z.string().min(1),
				subject: z.uuid(),
				occurred_at: z.iso.datetime({ offset: true }),
			})
			.transform(({ actor_id, occurred_at, ...event }) => ({
				...event,
				actorId: actor_id,
				occurredAt: occurred_at,
			})),
	)
	.max(workspacePageSize);

export function auditEventsKey(organizationId: string) {
	return ["workspace", "audit", organizationId] as const;
}

export function auditEvents(client: BackendClient, organizationId: string) {
	return infiniteQueryOptions({
		queryKey: auditEventsKey(organizationId),
		initialPageParam: "",
		gcTime: 0,
		queryFn: async ({ pageParam, signal }) =>
			events.parse(
				workspaceBody(
					await client.GET("/v1/organizations/{organization}/audit-events", {
						params: {
							path: { organization: organizationId },
							query: pageParam ? { after: pageParam } : {},
						},
						signal,
						cache: "no-store",
					}),
				),
			),
		getNextPageParam: (page) => nextIdCursor(page),
	});
}
