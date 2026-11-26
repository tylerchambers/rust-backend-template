import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import type { InvitationDraft } from "../domain/team";
import type { WorkspaceRole } from "../domain/workspace";
import { auditEventsKey } from "../infrastructure/audit";
import {
	createInvitation,
	invitationList,
	revokeInvitation,
} from "../infrastructure/invitations";
import {
	changeMemberRole,
	memberListKey,
	removeMember,
} from "../infrastructure/members";
import { organizationListKey } from "../infrastructure/organizations";
import { sessionListKey } from "../infrastructure/sessions";
import { useAccountSession } from "./accountSession";
import { useBackend } from "./backend";

export type MemberChange =
	| { action: "change_role"; role: WorkspaceRole }
	| { action: "remove_member" };

export function useChangeMember(organizationId: string, userId: string) {
	const client = useBackend();
	const cache = useQueryClient();
	const { account, recheck } = useAccountSession();
	return async (change: MemberChange, proof: string): Promise<void> => {
		if (change.action === "change_role") {
			await changeMemberRole(
				client,
				organizationId,
				userId,
				change.role,
				proof,
			);
		} else {
			await removeMember(client, organizationId, userId, proof);
		}
		const refresh = [
			cache.invalidateQueries({ queryKey: memberListKey(organizationId) }),
			cache.invalidateQueries({
				queryKey: auditEventsKey(organizationId),
			}),
			cache.invalidateQueries({ queryKey: organizationListKey }),
		];
		if (change.action === "change_role" && userId === account.id) {
			refresh.push(
				cache.invalidateQueries({ queryKey: sessionListKey }),
				recheck(),
			);
		}
		await Promise.all(refresh);
	};
}

export function useInvitationManagement(organizationId: string) {
	const client = useBackend();
	const cache = useQueryClient();
	const options = invitationList(client, organizationId);
	const history = useInfiniteQuery(options);
	async function refreshAfter(write: Promise<void>): Promise<void> {
		await write;
		void cache.invalidateQueries({ queryKey: options.queryKey });
		void cache.invalidateQueries({
			queryKey: auditEventsKey(organizationId),
		});
	}
	return {
		history,
		invite: (draft: InvitationDraft, proof: string) =>
			refreshAfter(createInvitation(client, organizationId, draft, proof)),
		revoke: (id: string, proof: string) =>
			refreshAfter(revokeInvitation(client, organizationId, id, proof)),
	};
}
