import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useBackend } from "../../application/backend";
import { workspaceName } from "../../domain/workspace";
import {
	type Project,
	projectDetail,
	projectList,
	renameProject,
} from "../../infrastructure/projects";
import {
	isVersionConflict,
	workspaceError,
} from "../../infrastructure/workspaceResponse";

type Conflict =
	| { kind: "pending"; fetching: boolean }
	| { kind: "failed"; message: string; retry: () => void }
	| { kind: "ready"; name: string; version: number; accept: () => void };

type ProjectRename = {
	base: Pick<Project, "name" | "version">;
	name: string;
	setName: (name: string) => void;
	nameError: string | undefined;
	canSave: boolean;
	isSaving: boolean;
	saveError: string | null;
	conflict: Conflict | null;
	save: () => void;
	cancel: () => void;
};

export function useProjectRename(
	project: Project,
	onSaved: (name: string) => void,
	onCancel: () => void,
): ProjectRename {
	const client = useBackend();
	const cache = useQueryClient();
	// List refetches must not replace the version against which this draft was made.
	const [base, setBase] = useState(project);
	const [name, setName] = useState(project.name);
	const parsed = workspaceName.safeParse(name);
	const rename = useMutation({
		mutationFn: (newName: string) => renameProject(client, base, newName),
		onSuccess: updateList,
	});
	const hasConflict = isVersionConflict(rename.error);
	const latest = useQuery({
		...projectDetail(client, project.organizationId, project.id),
		enabled: hasConflict,
		staleTime: 0,
	});
	const canSave = !hasConflict && parsed.success && name !== base.name;

	function updateList(updated: Project): void {
		cache.setQueryData(
			projectList(client, project.organizationId).queryKey,
			(old) =>
				old && {
					...old,
					pages: old.pages.map((page) =>
						page.map((item) =>
							item.id === updated.id && item.version <= updated.version
								? updated
								: item,
						),
					),
				},
		);
	}

	function conflictState(): Conflict | null {
		if (!hasConflict) return null;
		if (latest.isFetching) return { kind: "pending", fetching: true };
		if (latest.isError) {
			return {
				kind: "failed",
				message: workspaceError(latest.error),
				retry: () => void latest.refetch(),
			};
		}
		const current = latest.data;
		if (!current) return { kind: "pending", fetching: false };
		return {
			kind: "ready",
			name: current.name,
			version: current.version,
			accept: () => {
				setBase(current);
				updateList(current);
				rename.reset();
			},
		};
	}

	return {
		base,
		name,
		setName,
		nameError:
			name && !parsed.success ? parsed.error.issues[0]?.message : undefined,
		canSave,
		isSaving: rename.isPending,
		saveError:
			!hasConflict && rename.isError ? workspaceError(rename.error) : null,
		conflict: conflictState(),
		save: () => {
			if (canSave && !rename.isPending) {
				rename.mutate(parsed.data, {
					onSuccess: (updated) => onSaved(updated.name),
				});
			}
		},
		cancel: () => {
			if (hasConflict && latest.isSuccess && !latest.isFetching)
				updateList(latest.data);
			onCancel();
		},
	};
}
