#!/usr/bin/env node
import { existsSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { Command } from "commander";
import { closeBrowser, getBrowser, getStatus } from "./browser.js";
import { content, snapshot, text } from "./commands/content.js";
import {
	click,
	fill,
	focus,
	hover,
	press,
	select,
	type,
} from "./commands/interact.js";
import { back, forward, navigate, reload } from "./commands/navigate.js";
import { screenshot } from "./commands/screenshot.js";
import {
	waitForNavigation,
	waitForSelector,
	waitForText,
	waitForTimeout,
} from "./commands/wait.js";

const program = new Command();

function output(data: unknown): void {
	console.log(JSON.stringify(data, null, 2));
}

// Wrapper to ensure browser closes after command
async function withBrowser<T>(fn: () => Promise<T>): Promise<void> {
	try {
		const result = await fn();
		output(result);
	} finally {
		await closeBrowser();
	}
}

program
	.name("pw")
	.description("Playwright CLI for Pi Agent browser automation")
	.version("1.0.0");

// Navigation commands
program
	.command("navigate <url>")
	.alias("goto")
	.description("Navigate to a URL")
	.action(async (url: string) => {
		await withBrowser(() => navigate(url));
	});

program
	.command("back")
	.description("Go back in history")
	.action(async () => {
		await withBrowser(() => back());
	});

program
	.command("forward")
	.description("Go forward in history")
	.action(async () => {
		await withBrowser(() => forward());
	});

program
	.command("reload")
	.description("Reload the current page")
	.action(async () => {
		await withBrowser(() => reload());
	});

// Screenshot command
program
	.command("screenshot")
	.description("Take a screenshot")
	.option("-o, --output <path>", "Output file path")
	.option("-f, --full-page", "Capture full page", false)
	.action(async (options: { output?: string; fullPage?: boolean }) => {
		await withBrowser(() =>
			screenshot({ output: options.output, fullPage: options.fullPage }),
		);
	});

// Interaction commands
program
	.command("click <selector>")
	.description("Click an element")
	.action(async (selector: string) => {
		await withBrowser(() => click(selector));
	});

program
	.command("type <selector> <text>")
	.description("Type text into an element (appends)")
	.action(async (selector: string, textValue: string) => {
		await withBrowser(() => type(selector, textValue));
	});

program
	.command("fill <selector> <value>")
	.description("Fill an input with a value (replaces)")
	.action(async (selector: string, value: string) => {
		await withBrowser(() => fill(selector, value));
	});

program
	.command("select <selector> <value>")
	.description("Select an option from a dropdown")
	.action(async (selector: string, value: string) => {
		await withBrowser(() => select(selector, value));
	});

program
	.command("hover <selector>")
	.description("Hover over an element")
	.action(async (selector: string) => {
		await withBrowser(() => hover(selector));
	});

program
	.command("focus <selector>")
	.description("Focus an element")
	.action(async (selector: string) => {
		await withBrowser(() => focus(selector));
	});

program
	.command("press <key>")
	.description("Press a key (e.g., Enter, Tab, Escape)")
	.action(async (key: string) => {
		await withBrowser(() => press(key));
	});

// Content commands
program
	.command("content")
	.description("Get page content")
	.option("--format <format>", "Output format: text or html", "text")
	.action(async (options: { format: string }) => {
		const fmt = options.format === "html" ? "html" : "text";
		await withBrowser(() => content(fmt));
	});

program
	.command("text <selector>")
	.description("Get text content of an element")
	.action(async (selector: string) => {
		await withBrowser(() => text(selector));
	});

program
	.command("snapshot")
	.description("Get accessibility tree snapshot")
	.action(async () => {
		await withBrowser(() => snapshot());
	});

// Wait commands
program
	.command("wait-selector <selector>")
	.description("Wait for a selector to appear")
	.option("-t, --timeout <ms>", "Timeout in milliseconds", "30000")
	.action(async (selector: string, options: { timeout: string }) => {
		await withBrowser(() =>
			waitForSelector(selector, Number.parseInt(options.timeout, 10)),
		);
	});

program
	.command("wait-text <text>")
	.description("Wait for text to appear on page")
	.option("-t, --timeout <ms>", "Timeout in milliseconds", "30000")
	.action(async (textValue: string, options: { timeout: string }) => {
		await withBrowser(() =>
			waitForText(textValue, Number.parseInt(options.timeout, 10)),
		);
	});

program
	.command("wait-navigation")
	.description("Wait for navigation to complete")
	.option("-t, --timeout <ms>", "Timeout in milliseconds", "30000")
	.action(async (options: { timeout: string }) => {
		await withBrowser(() =>
			waitForNavigation(Number.parseInt(options.timeout, 10)),
		);
	});

program
	.command("wait <ms>")
	.description("Wait for specified milliseconds")
	.action(async (ms: string) => {
		await withBrowser(() => waitForTimeout(Number.parseInt(ms, 10)));
	});

// Composite command: fetch URL and get content/screenshot in one session
program
	.command("fetch <url>")
	.description("Navigate to URL and extract content or take screenshot")
	.option("-o, --output <path>", "Save screenshot to path")
	.option("-f, --full-page", "Full page screenshot", false)
	.option("--format <format>", "Content format: text or html", "text")
	.option("--screenshot", "Take screenshot instead of content")
	.action(
		async (
			url: string,
			options: {
				output?: string;
				fullPage?: boolean;
				format?: string;
				screenshot?: boolean;
			},
		) => {
			try {
				const { page } = await getBrowser();

				// Navigate
				let targetUrl = url;
				if (!url.startsWith("http://") && !url.startsWith("https://")) {
					targetUrl = `https://${url}`;
				}
				await page.goto(targetUrl, { waitUntil: "domcontentloaded" });

				const pageUrl = page.url();
				const title = await page.title();

				if (options.screenshot || options.output) {
					// Screenshot mode
					const outputPath =
						options.output || `/tmp/pw-screenshot-${Date.now()}.png`;

					// Ensure directory exists
					const dir = dirname(outputPath);
					if (!existsSync(dir)) {
						mkdirSync(dir, { recursive: true });
					}

					await page.screenshot({
						path: outputPath,
						fullPage: options.fullPage ?? false,
					});

					output({
						success: true,
						url: pageUrl,
						title,
						screenshot: outputPath,
						timestamp: new Date().toISOString(),
					});
				} else {
					// Content mode
					let contentStr: string;
					if (options.format === "html") {
						contentStr = await page.content();
					} else {
						contentStr = await page.evaluate(() => document.body.innerText);
					}

					output({
						success: true,
						url: pageUrl,
						title,
						content: contentStr,
						timestamp: new Date().toISOString(),
					});
				}
			} catch (err) {
				const error = err instanceof Error ? err.message : String(err);
				output({
					success: false,
					url,
					error,
					timestamp: new Date().toISOString(),
				});
			} finally {
				await closeBrowser();
			}
		},
	);

// Session commands
program
	.command("status")
	.description("Get browser session status")
	.action(async () => {
		output(await getStatus());
	});

program
	.command("close")
	.description("Close the browser session")
	.action(async () => {
		await closeBrowser();
		output({
			success: true,
			message: "Browser closed",
			timestamp: new Date().toISOString(),
		});
	});

program.parse();
