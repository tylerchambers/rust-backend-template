import { Link } from "@tanstack/react-router";

export function WorkspaceBreadcrumbs({
	current,
	organizationId,
	organizationName = "Projects",
	viaSettings = false,
}: {
	current: string;
	organizationId?: string | undefined;
	organizationName?: string | undefined;
	viaSettings?: boolean;
}): React.JSX.Element {
	return (
		<nav aria-label="Breadcrumb" className="workspace-trail">
			<ol>
				<li>
					<Link to="/" activeOptions={{ exact: true }}>
						<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round">
							<path d="m8 2 6 3-6 3-6-3 6-3Zm-6 6 6 3 6-3M2 11l6 3 6-3" />
						</svg>
						Organizations
					</Link>
				</li>
				{organizationId && (
					<li>
						<Link to="/organizations/$organizationId" params={{ organizationId }} activeOptions={{ exact: true }}>
							{organizationName}
						</Link>
					</li>
				)}
				{organizationId && viaSettings && (
					<li>
						<Link to="/organizations/$organizationId/settings" params={{ organizationId }}>
							Settings
						</Link>
					</li>
				)}
				<li><span aria-current="page">{current}</span></li>
			</ol>
		</nav>
	);
}
