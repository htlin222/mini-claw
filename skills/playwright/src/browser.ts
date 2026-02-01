import { homedir } from "node:os";
import { join } from "node:path";
import {
	type Browser,
	type BrowserContext,
	chromium,
	type Page,
} from "playwright";

const SESSION_DIR = join(homedir(), ".mini-claw", "playwright");

export interface BrowserSession {
	browser: Browser;
	context: BrowserContext;
	page: Page;
}

// Module-level browser instance for reuse within same process
let browserInstance: Browser | null = null;
let contextInstance: BrowserContext | null = null;
let pageInstance: Page | null = null;

async function launchBrowser(): Promise<Browser> {
	const browser = await chromium.launch({
		headless: true,
		args: ["--no-sandbox", "--disable-setuid-sandbox"],
	});
	return browser;
}

export async function getBrowser(): Promise<BrowserSession> {
	// Reuse existing browser if available
	if (browserInstance && browserInstance.isConnected()) {
		if (contextInstance && pageInstance) {
			return {
				browser: browserInstance,
				context: contextInstance,
				page: pageInstance,
			};
		}
	}

	// Launch new browser
	browserInstance = await launchBrowser();

	contextInstance = await browserInstance.newContext({
		viewport: { width: 1280, height: 720 },
		userAgent:
			"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
	});

	pageInstance = await contextInstance.newPage();

	return {
		browser: browserInstance,
		context: contextInstance,
		page: pageInstance,
	};
}

export async function closeBrowser(): Promise<void> {
	if (browserInstance) {
		await browserInstance.close();
		browserInstance = null;
		contextInstance = null;
		pageInstance = null;
	}
}

export async function getStatus(): Promise<{
	connected: boolean;
	url?: string;
	title?: string;
}> {
	if (!browserInstance || !browserInstance.isConnected()) {
		return { connected: false };
	}

	try {
		if (pageInstance) {
			const url = pageInstance.url();
			const title = await pageInstance.title();
			return {
				connected: true,
				url: url === "about:blank" ? undefined : url,
				title: title || undefined,
			};
		}
		return { connected: true };
	} catch {
		return { connected: false };
	}
}

export function getSessionDir(): string {
	return SESSION_DIR;
}
