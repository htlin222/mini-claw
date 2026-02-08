/**
 * Convert markdown to Telegram-compatible HTML
 * Supports: bold, italic, code, code blocks, links
 */

// Escape HTML special characters
function escapeHtml(text: string): string {
	return text
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;");
}

// Convert markdown to Telegram HTML
export function markdownToHtml(text: string): string {
	// Use null character as delimiter to avoid matching bold/italic patterns
	// The previous __CODE_BLOCK_0__ format was incorrectly matched by the
	// bold pattern /__([^_]+)__/g, causing placeholders to be converted to
	// <b>CODE_BLOCK_0</b> instead of being preserved for later restoration.
	const PLACEHOLDER_START = "\x00";
	const PLACEHOLDER_END = "\x00";

	// First, extract and preserve code blocks to prevent processing inside them
	const codeBlocks: string[] = [];
	let processed = text.replace(
		/```(\w*)\n?([\s\S]*?)```/g,
		(_, _lang, code) => {
			const index = codeBlocks.length;
			// Escape HTML inside code blocks
			codeBlocks.push(`<pre>${escapeHtml(code.trim())}</pre>`);
			return `${PLACEHOLDER_START}CODE_BLOCK_${index}${PLACEHOLDER_END}`;
		},
	);

	// Extract inline code
	const inlineCodes: string[] = [];
	processed = processed.replace(/`([^`]+)`/g, (_, code) => {
		const index = inlineCodes.length;
		inlineCodes.push(`<code>${escapeHtml(code)}</code>`);
		return `${PLACEHOLDER_START}INLINE_CODE_${index}${PLACEHOLDER_END}`;
	});

	// Escape HTML in remaining text
	processed = escapeHtml(processed);

	// Convert markdown formatting
	// Bold: **text** or __text__
	processed = processed.replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>");
	processed = processed.replace(/__([^_]+)__/g, "<b>$1</b>");

	// Italic: *text* or _text_ (but not inside words)
	processed = processed.replace(
		/(?<![a-zA-Z])\*([^*]+)\*(?![a-zA-Z])/g,
		"<i>$1</i>",
	);
	processed = processed.replace(
		/(?<![a-zA-Z])_([^_]+)_(?![a-zA-Z])/g,
		"<i>$1</i>",
	);

	// Strikethrough: ~~text~~
	processed = processed.replace(/~~([^~]+)~~/g, "<s>$1</s>");

	// Links: [text](url)
	processed = processed.replace(
		/\[([^\]]+)\]\(([^)]+)\)/g,
		'<a href="$2">$1</a>',
	);

	// Restore code blocks
	for (let i = 0; i < codeBlocks.length; i++) {
		processed = processed.replace(
			`${PLACEHOLDER_START}CODE_BLOCK_${i}${PLACEHOLDER_END}`,
			codeBlocks[i],
		);
	}

	// Restore inline code
	for (let i = 0; i < inlineCodes.length; i++) {
		processed = processed.replace(
			`${PLACEHOLDER_START}INLINE_CODE_${i}${PLACEHOLDER_END}`,
			inlineCodes[i],
		);
	}

	return processed;
}

// Strip all markdown formatting to plain text
export function stripMarkdown(text: string): string {
	return (
		text
			// Remove code blocks
			.replace(/```[\s\S]*?```/g, (match) => {
				const code = match.replace(/```\w*\n?/, "").replace(/```$/, "");
				return code.trim();
			})
			// Remove inline code backticks
			.replace(/`([^`]+)`/g, "$1")
			// Remove bold
			.replace(/\*\*([^*]+)\*\*/g, "$1")
			.replace(/__([^_]+)__/g, "$1")
			// Remove italic
			.replace(/\*([^*]+)\*/g, "$1")
			.replace(/_([^_]+)_/g, "$1")
			// Remove strikethrough
			.replace(/~~([^~]+)~~/g, "$1")
			// Convert links to just text
			.replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
	);
}
