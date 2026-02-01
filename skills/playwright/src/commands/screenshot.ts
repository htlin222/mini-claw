import { homedir } from "node:os";
import { join } from "node:path";
import { getBrowser } from "../browser.js";

export interface ScreenshotResult {
	success: boolean;
	path: string;
	url: string;
	timestamp: string;
	error?: string;
}

export interface ScreenshotOptions {
	output?: string;
	fullPage?: boolean;
}

export async function screenshot(
	options: ScreenshotOptions = {},
): Promise<ScreenshotResult> {
	try {
		const { page } = await getBrowser();

		const outputPath =
			options.output ||
			join(
				homedir(),
				".mini-claw",
				"playwright",
				`screenshot-${Date.now()}.png`,
			);

		await page.screenshot({
			path: outputPath,
			fullPage: options.fullPage ?? false,
		});

		return {
			success: true,
			path: outputPath,
			url: page.url(),
			timestamp: new Date().toISOString(),
		};
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return {
			success: false,
			path: options.output || "",
			url: "",
			timestamp: new Date().toISOString(),
			error,
		};
	}
}
