import { infiniteQueryOptions } from "@tanstack/react-query";
import type { BackendClient } from "rust-backend-template-sdk";
import { z } from "zod";
import { workspaceBody } from "./workspace";

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
	.max(50);

export function auditEvents(client: BackendClient, organizationId: string) {
	return infiniteQueryOptions({
		queryKey: ["workspace", "audit", organizationId] as const,
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
		getNextPageParam: (page) =>
			page.length === 50 ? page.at(-1)?.id : undefined,
	});
}
