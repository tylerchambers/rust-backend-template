export const workspacePageSize = 50;

export function nextIdCursor(
	page: readonly { id: string }[],
): string | undefined {
	return page.length === workspacePageSize ? page.at(-1)?.id : undefined;
}
