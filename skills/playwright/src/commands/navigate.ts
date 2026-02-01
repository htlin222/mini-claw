import { getBrowser } from "../browser.js";

export interface NavigateResult {
	success: boolean;
	url: string;
	title: string;
	timestamp: string;
	error?: string;
}

function result(
	success: boolean,
	url: string,
	title: string,
	error?: string,
): NavigateResult {
	return {
		success,
		url,
		title,
		timestamp: new Date().toISOString(),
		...(error && { error }),
	};
}

export async function navigate(url: string): Promise<NavigateResult> {
	try {
		const { page } = await getBrowser();

		// Add protocol if missing
		let targetUrl = url;
		if (!url.startsWith("http://") && !url.startsWith("https://")) {
			targetUrl = `https://${url}`;
		}

		await page.goto(targetUrl, { waitUntil: "domcontentloaded" });
		const title = await page.title();

		return result(true, page.url(), title);
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, url, "", error);
	}
}

export async function back(): Promise<NavigateResult> {
	try {
		const { page } = await getBrowser();
		await page.goBack({ waitUntil: "domcontentloaded" });
		const title = await page.title();

		return result(true, page.url(), title);
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", "", error);
	}
}

export async function forward(): Promise<NavigateResult> {
	try {
		const { page } = await getBrowser();
		await page.goForward({ waitUntil: "domcontentloaded" });
		const title = await page.title();

		return result(true, page.url(), title);
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", "", error);
	}
}

export async function reload(): Promise<NavigateResult> {
	try {
		const { page } = await getBrowser();
		await page.reload({ waitUntil: "domcontentloaded" });
		const title = await page.title();

		return result(true, page.url(), title);
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", "", error);
	}
}
