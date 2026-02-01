import { getBrowser } from "../browser.js";

export interface InteractResult {
	success: boolean;
	url: string;
	timestamp: string;
	error?: string;
}

function result(success: boolean, url: string, error?: string): InteractResult {
	return {
		success,
		url,
		timestamp: new Date().toISOString(),
		...(error && { error }),
	};
}

export async function click(selector: string): Promise<InteractResult> {
	try {
		const { page } = await getBrowser();
		await page.click(selector, { timeout: 10000 });

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}

export async function type(
	selector: string,
	text: string,
): Promise<InteractResult> {
	try {
		const { page } = await getBrowser();
		await page.type(selector, text, { timeout: 10000 });

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}

export async function fill(
	selector: string,
	value: string,
): Promise<InteractResult> {
	try {
		const { page } = await getBrowser();
		await page.fill(selector, value, { timeout: 10000 });

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}

export async function select(
	selector: string,
	value: string,
): Promise<InteractResult> {
	try {
		const { page } = await getBrowser();
		await page.selectOption(selector, value, { timeout: 10000 });

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}

export async function hover(selector: string): Promise<InteractResult> {
	try {
		const { page } = await getBrowser();
		await page.hover(selector, { timeout: 10000 });

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}

export async function focus(selector: string): Promise<InteractResult> {
	try {
		const { page } = await getBrowser();
		await page.focus(selector, { timeout: 10000 });

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}

export async function press(key: string): Promise<InteractResult> {
	try {
		const { page } = await getBrowser();
		await page.keyboard.press(key);

		return result(true, page.url());
	} catch (err) {
		const error = err instanceof Error ? err.message : String(err);
		return result(false, "", error);
	}
}
