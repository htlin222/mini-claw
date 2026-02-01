import { getBrowser } from "../browser.js";

export interface ContentResult {
	success: boolean;
	content: string;
	url: string;
	timestamp: string;
	error?: string;
}

export interface TextResult {
	success: boolean;
	text: string;
	url: string;
	timestamp: string;
	error?: string;
}

export interface SnapshotResult {
	success: boolean;
	snapshot: string;
	url: string;
	timestamp: string;
	error?: string;
}

export async function content(
	format: "text" | "html" = "text",
): Promise<ContentResult> {
	try {
		const { page } = await getBrowser();

		let contentStr: string;
		if (format === "html") {
			contentStr = await page.content();
		} else {
			contentStr = await page.evaluate(() => document.body.innerText);
		}

		return {
			success: true,
			content: contentStr,
			url: page.url(),
			timestamp: new Date().toISOString(),
		};
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return {
			success: false,
			content: "",
			url: "",
			timestamp: new Date().toISOString(),
			error,
		};
	}
}

export async function text(selector: string): Promise<TextResult> {
	try {
		const { page } = await getBrowser();
		const element = await page.locator(selector).first();
		const textContent = (await element.textContent()) || "";

		return {
			success: true,
			text: textContent.trim(),
			url: page.url(),
			timestamp: new Date().toISOString(),
		};
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return {
			success: false,
			text: "",
			url: "",
			timestamp: new Date().toISOString(),
			error,
		};
	}
}

export async function snapshot(): Promise<SnapshotResult> {
	try {
		const { page } = await getBrowser();
		// Use ariaSnapshot for accessibility tree (Playwright 1.49+)
		const tree = await page.locator("body").ariaSnapshot();

		return {
			success: true,
			snapshot: tree,
			url: page.url(),
			timestamp: new Date().toISOString(),
		};
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return {
			success: false,
			snapshot: "",
			url: "",
			timestamp: new Date().toISOString(),
			error,
		};
	}
}
