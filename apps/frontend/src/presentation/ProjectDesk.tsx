import { Box, Container, Group } from "@mantine/core";
import type { JSX, ReactNode } from "react";

export function ProjectDesk({
	homeHref = "#/",
	profile,
	children,
}: {
	homeHref?: string;
	profile?: ReactNode;
	children: ReactNode;
}): JSX.Element {
	return (
		<>
			<button
				type="button"
				className="skip-link"
				onClick={() => document.getElementById("workspace")?.focus()}
			>
				Skip to content
			</button>
			<Box component="header" className="masthead">
				<Container size="md">
					<Group justify="space-between" py="sm">
						<a href={homeHref} className="wordmark">
							Project desk
						</a>
						{profile}
					</Group>
				</Container>
			</Box>
			<Container
				component="main"
				size="md"
				py="lg"
				id="workspace"
				tabIndex={-1}
			>
				{children}
			</Container>
		</>
	);
}
