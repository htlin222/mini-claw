import { getBrowser } from "../browser.js";

export interface WaitResult {
	success: boolean;
	url: string;
	timestamp: string;
	error?: string;
}

function result(success: boolean, url: string, error?: string): WaitResult {
	return {
		success,
		url,
		timestamp: new Date().toISOString(),
		...(error && { error }),
	};
}

export async function waitForSelector(
	selector: string,
	timeout = 30000,
): Promise<WaitResult> {
	try {
		const { page } = await getBrowser();
		await page.waitForSelector(selector, { timeout });

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}

export async function waitForText(
	text: string,
	timeout = 30000,
): Promise<WaitResult> {
	try {
		const { page } = await getBrowser();
		await page.waitForFunction(
			(searchText: string) => document.body.innerText.includes(searchText),
			text,
			{ timeout },
		);

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}

export async function waitForNavigation(timeout = 30000): Promise<WaitResult> {
	try {
		const { page } = await getBrowser();
		await page.waitForNavigation({ timeout });

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}

export async function waitForTimeout(ms: number): Promise<WaitResult> {
	try {
		const { page } = await getBrowser();
		await page.waitForTimeout(ms);

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}
