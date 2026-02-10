import { spawn } from "node:child_process";
import { mkdir, unlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Bot, InlineKeyboard, InputFile } from "grammy";
import type { Config } from "./config.js";
import { detectFiles, snapshotWorkspace } from "./file-detector.js";
import { markdownToHtml, stripMarkdown } from "./markdown.js";
import {
	type ActivityUpdate,
	type RunPiOptions,
	type ExtractedImage,
	acquireLock,
	checkPiAuth,
	extractImagesFromSession,
	getSessionLineCount,
	runPiWithStreaming,
} from "./pi-runner.js";
import { checkRateLimit } from "./rate-limiter.js";
import {
	archiveSession,
	cleanupOldSessions,
	clearActiveSession,
	formatFileSize,
	formatSessionAge,
	generateSessionTitle,
	listSessions,
	switchSession,
} from "./sessions.js";
import { formatPath, getWorkspace, setWorkspace } from "./workspace.js";

interface ShellResult {
	stdout: string;
	stderr: string;
	code: number | null;
}

async function runShell(
	cmd: string,
	cwd: string,
	timeoutMs: number,
): Promise<ShellResult> {
	return new Promise((resolve) => {
		const proc = spawn("bash", ["-c", cmd], {
			cwd,
			env: process.env,
			stdio: ["ignore", "pipe", "pipe"],
		});

		let stdout = "";
		let stderr = "";

		proc.stdout.on("data", (data) => {
			stdout += data.toString();
		});

		proc.stderr.on("data", (data) => {
			stderr += data.toString();
		});

		proc.on("close", (code) => {
			resolve({ stdout, stderr, code });
		});

		proc.on("error", (err) => {
			resolve({ stdout: "", stderr: err.message, code: 1 });
		});

		setTimeout(() => {
			proc.kill("SIGTERM");
			resolve({ stdout, stderr: `${stderr}\n(timeout)`, code: 124 });
		}, timeoutMs);
	});
}

const MAX_MESSAGE_LENGTH = 4096;

function splitMessage(text: string): string[] {
	if (text.length <= MAX_MESSAGE_LENGTH) {
		return [text];
	}

	const chunks: string[] = [];
	let remaining = text;

	while (remaining.length > 0) {
		if (remaining.length <= MAX_MESSAGE_LENGTH) {
			chunks.push(remaining);
			break;
		}

		// Try to split at newline
		let splitIndex = remaining.lastIndexOf("\n", MAX_MESSAGE_LENGTH);
		if (splitIndex === -1 || splitIndex < MAX_MESSAGE_LENGTH / 2) {
			// Fall back to space
			splitIndex = remaining.lastIndexOf(" ", MAX_MESSAGE_LENGTH);
		}
		if (splitIndex === -1 || splitIndex < MAX_MESSAGE_LENGTH / 2) {
			// Hard split
			splitIndex = MAX_MESSAGE_LENGTH;
		}

		chunks.push(remaining.slice(0, splitIndex));
		remaining = remaining.slice(splitIndex).trimStart();
	}

	return chunks;
}

export function createBot(config: Config): Bot {
	const bot = new Bot(config.telegramToken);

	// Access control middleware
	if (config.allowedUsers.length > 0) {
		bot.use(async (ctx, next) => {
			const userId = ctx.from?.id;
			if (userId && config.allowedUsers.includes(userId)) {
				await next();
			} else {
				await ctx.reply("Sorry, you are not authorized to use this bot.");
			}
		});
	}

	// Command descriptions for menu
	const commands = [
		{ command: "start", description: "Welcome & quick start" },
		{ command: "help", description: "Show all commands" },
		{ command: "pwd", description: "Show current directory" },
		{ command: "cd", description: "Change directory" },
		{ command: "home", description: "Go to home directory" },
		{ command: "shell", description: "Run shell command" },
		{ command: "session", description: "Manage sessions" },
		{ command: "new", description: "Start fresh conversation" },
		{ command: "status", description: "Show bot status" },
	];

	// Set bot command menu
	bot.api.setMyCommands(commands).catch(() => {
		// Ignore errors (might not have permission)
	});

	// /start command
	bot.command("start", async (ctx) => {
		const piOk = await checkPiAuth();
		const status = piOk
			? "Pi is ready"
			: "Pi is not installed or not authenticated";
		const cwd = await getWorkspace(ctx.chat.id);

		await ctx.reply(
			`Welcome to Mini-Claw!

${status}
Working directory: ${formatPath(cwd)}

Type /help for all commands.
Send any message to chat with AI.`,
		);
	});

	// /help command
	bot.command("help", async (ctx) => {
		await ctx.reply(
			`📖 Mini-Claw Commands

📁 Navigation:
/pwd - Show current directory
/cd <path> - Change directory
/home - Go to home directory

🔧 Execution:
/shell <cmd> - Run shell command directly

💬 Sessions:
/session - List & manage sessions
/new - Archive current & start fresh

📊 Info:
/status - Show bot status
/help - Show this message

💡 Tips:
• Any text → AI conversation
• /shell runs instantly, no AI
• /cd supports ~, .., relative paths`,
		);
	});

	// /pwd command
	bot.command("pwd", async (ctx) => {
		const cwd = await getWorkspace(ctx.chat.id);
		await ctx.reply(`📁 ${formatPath(cwd)}`);
	});

	// /home command
	bot.command("home", async (ctx) => {
		try {
			const cwd = await setWorkspace(ctx.chat.id, "~");
			await ctx.reply(`📁 ${formatPath(cwd)}`);
		} catch (err) {
			const msg = err instanceof Error ? err.message : "Unknown error";
			await ctx.reply(`Error: ${msg}`);
		}
	});

	// /cd command
	bot.command("cd", async (ctx) => {
		const path = ctx.match?.trim();
		if (!path) {
			// No argument = go home
			try {
				const cwd = await setWorkspace(ctx.chat.id, "~");
				await ctx.reply(`📁 ${formatPath(cwd)}`);
			} catch (err) {
				const msg = err instanceof Error ? err.message : "Unknown error";
				await ctx.reply(`Error: ${msg}`);
			}
			return;
		}

		try {
			const cwd = await setWorkspace(ctx.chat.id, path);
			await ctx.reply(`📁 ${formatPath(cwd)}`);
		} catch (err) {
			const msg = err instanceof Error ? err.message : "Unknown error";
			await ctx.reply(`Error: ${msg}`);
		}
	});

	// /new command - start fresh session
	bot.command("new", async (ctx) => {
		// Must wait for any ongoing Pi execution to complete first
		// Otherwise Pi will recreate the session file after we rename it
		const release = await acquireLock(ctx.chat.id);
		try {
			const archived = await archiveSession(config, ctx.chat.id);
			// Clear active session tracking for truly fresh start
			await clearActiveSession(ctx.chat.id);
			if (archived) {
				await ctx.reply(
					`Session archived as ${archived}\nStarting fresh conversation.`,
				);
			} else {
				await ctx.reply("Starting fresh conversation.");
			}
		} finally {
			release();
		}
	});

	// /status command
	bot.command("status", async (ctx) => {
		const piOk = await checkPiAuth();
		const cwd = await getWorkspace(ctx.chat.id);
		await ctx.reply(
			`Status:
- Pi: ${piOk ? "OK" : "Not available"}
- Chat ID: ${ctx.chat.id}
- Workspace: ${formatPath(cwd)}`,
		);
	});

	// /shell command - run shell command in current directory
	bot.command("shell", async (ctx) => {
		const cmd = ctx.match?.trim();
		if (!cmd) {
			await ctx.reply("Usage: /shell <command>\nExample: /shell ls -la");
			return;
		}

		const cwd = await getWorkspace(ctx.chat.id);
		await ctx.replyWithChatAction("typing");

		try {
			const result = await runShell(cmd, cwd, config.shellTimeoutMs);

			let output = "";
			if (result.stdout) {
				output += result.stdout;
			}
			if (result.stderr) {
				output += `${output ? "\n" : ""}stderr: ${result.stderr}`;
			}
			if (!output) {
				output = "(no output)";
			}

			// Add exit code if non-zero
			if (result.code !== 0) {
				output += `\n\n[exit code: ${result.code}]`;
			}

			// Split long output
			const chunks = splitMessage(output.trim());
			for (const chunk of chunks) {
				await ctx.reply(chunk);
			}
		} catch (err) {
			const msg = err instanceof Error ? err.message : "Unknown error";
			await ctx.reply(`Error: ${msg}`);
		}
	});

	// /session command - list and manage sessions
	bot.command("session", async (ctx) => {
		await ctx.replyWithChatAction("typing");

		const sessions = await listSessions(config);

		if (sessions.length === 0) {
			await ctx.reply("No sessions found.");
			return;
		}

		// Generate titles for sessions (in parallel, max 5)
		const sessionsWithTitles = await Promise.all(
			sessions.slice(0, 10).map(async (session) => {
				const title = await generateSessionTitle(
					session.path,
					config.sessionTitleTimeoutMs,
				);
				return { ...session, title };
			}),
		);

		// Build inline keyboard
		const keyboard = new InlineKeyboard();

		for (const session of sessionsWithTitles) {
			const age = formatSessionAge(session.modifiedAt);
			const size = formatFileSize(session.sizeBytes);
			const label = `${session.title} (${age}, ${size})`;

			// Callback data format: session:load:<filename>
			keyboard.text(label, `session:load:${session.filename}`).row();
		}

		// Add cleanup button
		keyboard.text("🗑 Clean Up Old Sessions", "session:cleanup").row();

		await ctx.reply(
			`📚 Sessions (${sessions.length} total)\n\nTap to switch session:`,
			{ reply_markup: keyboard },
		);
	});

	// Handle callback queries for session buttons
	bot.callbackQuery(/^session:load:(.+)$/, async (ctx) => {
		const filename = ctx.match[1];
		const chatId = ctx.chat?.id;

		if (!chatId) {
			await ctx.answerCallbackQuery({ text: "Error: No chat ID" });
			return;
		}

		try {
			await switchSession(config, chatId, filename);
			await ctx.answerCallbackQuery({ text: "Session switched!" });
			await ctx.editMessageText(`✅ Switched to session: ${filename}`);
		} catch (err) {
			const msg = err instanceof Error ? err.message : "Unknown error";
			await ctx.answerCallbackQuery({
				text: `Error: ${msg}`,
				show_alert: true,
			});
		}
	});

	bot.callbackQuery("session:cleanup", async (ctx) => {
		await ctx.answerCallbackQuery({ text: "Cleaning up..." });

		const deleted = await cleanupOldSessions(config, 5);

		await ctx.editMessageText(
			`🗑 Cleanup complete!\nDeleted ${deleted} old session(s).\nKept the 5 most recent sessions per chat.`,
		);
	});

	// Handle all text messages
	bot.on("message:text", async (ctx) => {
		const chatId = ctx.chat.id;
		const text = ctx.message.text;

		// Skip commands
		if (text.startsWith("/")) {
			return;
		}

		// Rate limiting check
		const rateLimit = checkRateLimit(chatId, config.rateLimitCooldownMs);
		if (!rateLimit.allowed) {
			const seconds = Math.ceil((rateLimit.retryAfterMs || 0) / 1000);
			await ctx.reply(
				`⏳ Please wait ${seconds}s before sending another message.`,
			);
			return;
		}

		// Get current workspace for this chat
		const workspace = await getWorkspace(chatId);

		// Snapshot workspace before Pi execution
		const beforeSnapshot = await snapshotWorkspace(workspace);

		// Track session file line count to detect new images
		const sessionLinesBefore = await getSessionLineCount(config, chatId);

		// Send initial status message that we'll update
		const statusMsg = await ctx.reply("🔄 Working...");
		let lastStatusUpdate = Date.now();

		// Activity emoji mapping
		const activityEmoji: Record<string, string> = {
			thinking: "🧠",
			reading: "📖",
			writing: "✍️",
			running: "⚡",
			searching: "🔍",
			working: "🔄",
		};

		// Format status message
		const formatStatus = (activity: ActivityUpdate): string => {
			const emoji = activityEmoji[activity.type] || "🔄";
			const elapsed = `${activity.elapsed}s`;
			const detail = activity.detail ? `\n└─ ${activity.detail}` : "";
			return `${emoji} Working... (${elapsed})${detail}`;
		};

		// Activity callback - update status message
		const onActivity = async (activity: ActivityUpdate) => {
			// Throttle updates to avoid rate limits (max once per 2 seconds)
			const now = Date.now();
			if (now - lastStatusUpdate < 2000) return;
			lastStatusUpdate = now;

			try {
				await ctx.api.editMessageText(
					chatId,
					statusMsg.message_id,
					formatStatus(activity),
				);
			} catch {
				// Ignore edit errors (message might be deleted, or content unchanged)
			}
		};

		// Keep typing indicator active
		const typingInterval = setInterval(() => {
			ctx.replyWithChatAction("typing").catch(() => {});
		}, 4000);

		try {
			const result = await runPiWithStreaming(
				config,
				chatId,
				text,
				workspace,
				onActivity,
			);

			clearInterval(typingInterval);

			// Delete status message
			try {
				await ctx.api.deleteMessage(chatId, statusMsg.message_id);
			} catch {
				// Ignore delete errors
			}

			if (result.error) {
				await ctx.reply(`Error: ${result.error}`);
			}

			if (result.output) {
				// Try to send as HTML, fallback to plain text
				const chunks = splitMessage(result.output.trim());
				for (const chunk of chunks) {
					try {
						const html = markdownToHtml(chunk);
						await ctx.reply(html, { parse_mode: "HTML" });
					} catch {
						// Fallback to plain text if HTML fails
						await ctx.reply(stripMarkdown(chunk));
					}
				}
			}

			// Extract and send images from tool results (e.g., design_poster)
			const toolImages = await extractImagesFromSession(
				config,
				chatId,
				sessionLinesBefore,
			);

			for (let i = 0; i < toolImages.length; i++) {
				const img = toolImages[i];
				try {
					// Determine file extension from mimeType
					const ext = img.mimeType.split("/")[1] || "png";
					const filename = `image_${i + 1}.${ext}`;
					await ctx.replyWithPhoto(new InputFile(img.data, filename));
				} catch (err) {
					// Log error but don't spam user
					console.error("Failed to send tool image:", err);
				}
			}

			// Detect and send any files created by Pi (from filesystem)
			const detectedFiles = await detectFiles(
				result.output || "",
				workspace,
				beforeSnapshot,
			);

			for (const file of detectedFiles) {
				try {
					if (file.type === "photo") {
						await ctx.replyWithPhoto(new InputFile(file.path), {
							caption: file.filename,
						});
					} else {
						await ctx.replyWithDocument(new InputFile(file.path), {
							caption: file.filename,
						});
					}
				} catch {
					// File might have been deleted or inaccessible
					await ctx.reply(`(Could not send file: ${file.filename})`);
				}
			}
		} catch (err) {
			clearInterval(typingInterval);
			// Try to delete status message on error too
			try {
				await ctx.api.deleteMessage(chatId, statusMsg.message_id);
			} catch {
				// Ignore
			}
			const errorMsg = err instanceof Error ? err.message : "Unknown error";
			await ctx.reply(`Failed to process: ${errorMsg}`);
		}
	});

	// Photo message handler
	bot.on("message:photo", async (ctx) => {
		const chatId = ctx.chat.id;
		const userId = ctx.from?.id;

		// Check allowed users
		if (
			config.allowedUsers.length > 0 &&
			(!userId || !config.allowedUsers.includes(userId))
		) {
			await ctx.reply("Sorry, you are not authorized to use this bot.");
			return;
		}

		// Rate limiting
		const rateLimit = checkRateLimit(chatId, config.rateLimitCooldownMs);
		if (!rateLimit.allowed) {
			await ctx.reply(
				`Please wait ${Math.ceil((rateLimit.retryAfterMs || 0) / 1000)}s before sending another message.`,
			);
			return;
		}

		// Get caption or use default prompt
		const caption = ctx.message.caption || "What's in this image?";

		// Get the largest photo (last in array)
		const photos = ctx.message.photo;
		const largestPhoto = photos[photos.length - 1];

		// Download the photo
		let tempImagePath: string | null = null;
		try {
			const file = await ctx.api.getFile(largestPhoto.file_id);
			if (!file.file_path) {
				await ctx.reply("Failed to get image file path.");
				return;
			}

			// Download file content
			const fileUrl = `https://api.telegram.org/file/bot${config.telegramToken}/${file.file_path}`;
			const response = await fetch(fileUrl);
			if (!response.ok) {
				await ctx.reply("Failed to download image.");
				return;
			}
			const imageBuffer = Buffer.from(await response.arrayBuffer());

			// Save to temp file
			const tempDir = join(tmpdir(), "mini-claw");
			await mkdir(tempDir, { recursive: true });
			const ext = file.file_path.split(".").pop() || "jpg";
			tempImagePath = join(tempDir, `${chatId}-${Date.now()}.${ext}`);
			await writeFile(tempImagePath, imageBuffer);

			// Send status message
			const statusMsg = await ctx.reply("🔄 Analyzing image...");

			// Get workspace
			const workspace = (await getWorkspace(chatId)) || config.workspace;

			// Snapshot workspace for file detection
			const beforeSnapshot = await snapshotWorkspace(workspace);

			// Activity update callback
			const onActivity = (activity: ActivityUpdate) => {
				const icons: Record<ActivityUpdate["type"], string> = {
					thinking: "🤔",
					reading: "📖",
					writing: "✍️",
					running: "⚙️",
					searching: "🔍",
					working: "🔄",
				};
				const icon = icons[activity.type] || "🔄";
				const detail = activity.detail ? ` ${activity.detail}` : "";
				const elapsed = activity.elapsed > 0 ? ` (${activity.elapsed}s)` : "";

				ctx.api
					.editMessageText(
						chatId,
						statusMsg.message_id,
						`${icon}${detail}${elapsed}`,
					)
					.catch(() => {});
			};

			// Run Pi with image
			const result = await runPiWithStreaming(
				config,
				chatId,
				caption,
				workspace,
				onActivity,
				{ imagePaths: [tempImagePath] },
			);

			// Delete status message
			try {
				await ctx.api.deleteMessage(chatId, statusMsg.message_id);
			} catch {
				// Ignore
			}

			// Send error if any
			if (result.error) {
				await ctx.reply(`Error: ${result.error}`);
			}

			// Send output
			if (result.output) {
				const chunks = splitMessage(result.output.trim());
				for (const chunk of chunks) {
					try {
						const html = markdownToHtml(chunk);
						await ctx.reply(html, { parse_mode: "HTML" });
					} catch {
						await ctx.reply(stripMarkdown(chunk));
					}
				}
			}

			// Detect and send any files created by Pi
			const detectedFiles = await detectFiles(
				result.output || "",
				workspace,
				beforeSnapshot,
			);

			for (const file of detectedFiles) {
				try {
					if (file.type === "photo") {
						await ctx.replyWithPhoto(new InputFile(file.path), {
							caption: file.filename,
						});
					} else {
						await ctx.replyWithDocument(new InputFile(file.path), {
							caption: file.filename,
						});
					}
				} catch {
					await ctx.reply(`(Could not send file: ${file.filename})`);
				}
			}
		} catch (err) {
			const errorMsg = err instanceof Error ? err.message : "Unknown error";
			await ctx.reply(`Failed to process image: ${errorMsg}`);
		} finally {
			// Clean up temp image
			if (tempImagePath) {
				try {
					await unlink(tempImagePath);
				} catch {
					// Ignore cleanup errors
				}
			}
		}
	});

	return bot;
}
