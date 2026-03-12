/**
 * Hashline Tools — hash-aware file reading and editing for pi
 *
 * Wraps the hashline-tools Rust CLI to provide:
 * - hashread: Read files with hash anchors (LINE#HASH:content format)
 * - hashedit: Edit files using hash-aware operations (replace, append, prepend)
 *
 * Uses a full custom TUI component for displaying hashread output with
 * syntax highlighting and navigation.
 *
 * Based on: /home/gary/dev/hashline-tools
 */

import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import {
	type Component,
	Key,
	matchesKey,
	truncateToWidth,
	Text,
} from "@mariozechner/pi-tui";

import { Type } from "@sinclair/typebox";

// ─── Types ────────────────────────────────────────────────────────────────────

interface HashlineReadDetails {
	path: string;
	lineCount: number;
	offset: number;
	limit: number;
	truncated: boolean;
}

interface HashlineEditDetails {
	path: string;
	firstChangedLine: number | null;
	operations: number;
	success: boolean;
	error?: string;
}

interface HashlineDiffLine {
	sign: " " | "+" | "-";
	lineNum: number;
	hash: string | null;
	content: string;
}

// ─── Hashread Output Component ───────────────────────────────────────────────

class HashreadOutputComponent implements Component {
	private cachedLines: string[] | undefined;
	private scrollOffset: number = 0;
	private selectedLine: number | null = null;

	constructor(
		private tui: any,
		private theme: any,
		private content: string,
		private path: string,
		private done: (copiedLine: string | null) => void,
	) {
		// Parse the content to extract lines
	}

	private refresh() {
		this.cachedLines = undefined;
		this.tui.requestRender();
	}

	private parseHashreadContent(content: string): HashlineDiffLine[] {
		const lines: HashlineDiffLine[] = [];
		const fileMatch = content.match(/<file>\n([\s\S]*?)\n<\/file>/);
		if (!fileMatch) return [];

		const fileContent = fileMatch[1].trim();
		const rawLines = fileContent.split("\n");

		for (const rawLine of rawLines) {
			// Format: LINE#HASH:content
			const match = rawLine.match(/^(\d+)#([A-Z]{2}):(.*)$/);
			if (match) {
				lines.push({
					sign: " ",
					lineNum: parseInt(match[1], 10),
					hash: match[2],
					content: match[3],
				});
			}
		}

		return lines;
	}

	handleInput(data: string): void {
		const lines = this.parseHashreadContent(this.content);

		if (matchesKey(data, Key.escape)) {
			this.done(null);
			return;
		}

		if (matchesKey(data, Key.enter)) {
			if (this.selectedLine !== null && this.selectedLine < lines.length) {
				const line = lines[this.selectedLine];
				const anchor = `${line.lineNum}#${line.hash}`;
				this.done(anchor);
				return;
			}
			return;
		}

		if (data === "j" || matchesKey(data, Key.down)) {
			if (this.selectedLine === null) {
				this.selectedLine = 0;
			} else if (this.selectedLine < lines.length - 1) {
				this.selectedLine++;
			}
			this.refresh();
			return;
		}

		if (data === "k" || matchesKey(data, Key.up)) {
			if (this.selectedLine !== null && this.selectedLine > 0) {
				this.selectedLine--;
			}
			this.refresh();
			return;
		}

		if (matchesKey(data, Key.pageDown)) {
			if (this.selectedLine === null) {
				this.selectedLine = 24;
			} else {
				this.selectedLine = Math.min(this.selectedLine + 24, lines.length - 1);
			}
			this.refresh();
			return;
		}

		if (matchesKey(data, Key.pageUp)) {
			if (this.selectedLine !== null) {
				this.selectedLine = Math.max(this.selectedLine - 24, 0);
			}
			this.refresh();
			return;
		}

		if (data === "g") {
			this.selectedLine = 0;
			this.refresh();
			return;
		}

		if (data === "G") {
			this.selectedLine = lines.length - 1;
			this.refresh();
			return;
		}
	}

	render(width: number): string[] {
		if (this.cachedLines) return this.cachedLines;
		const lines: string[] = [];
		const add = (s: string) => lines.push(truncateToWidth(s, width));

		const parsedLines = this.parseHashreadContent(this.content);
		const totalLines = parsedLines.length;

		// Header
		add(this.theme.fg("accent", "─".repeat(width)));
		add(this.theme.fg("dim", ` hashread · ${this.path}`));
		lines.push("");

		if (parsedLines.length === 0) {
			add(this.theme.fg("muted", "  (empty or no lines)"));
			lines.push("");
			add(this.theme.fg("dim", " esc: close"));
			add(this.theme.fg("accent", "─".repeat(width)));
			this.cachedLines = lines;
			return lines;
		}

		// Calculate visible range
		const visibleLines = Math.min(20, totalLines);
		let startIdx = 0;
		if (this.selectedLine !== null) {
			startIdx = Math.max(0, Math.min(this.selectedLine - 10, totalLines - visibleLines));
		}
		const endIdx = Math.min(startIdx + visibleLines, totalLines);

		// Display lines
		for (let i = startIdx; i < endIdx; i++) {
			const line = parsedLines[i];
			const isSelected = this.selectedLine === i;

			let lineText = "";

			if (isSelected) {
				lineText += this.theme.fg("accent", "▶ ");
			} else {
				lineText += "  ";
			}

			// Line number and hash
			const anchor = `${line.lineNum}#${line.hash}`;
			lineText += this.theme.fg("muted", anchor.padEnd(10));
			lineText += this.theme.fg("text", line.content);

			add(lineText);
		}

		// Footer info
		lines.push("");
		const rangeInfo = totalLines > visibleLines ? `${startIdx + 1}-${endIdx}/${totalLines}` : `${totalLines} lines`;
		add(this.theme.fg("dim", ` ${rangeInfo}`));

		// Key hints
		if (this.selectedLine !== null) {
			const selectedLine = parsedLines[this.selectedLine];
			add(this.theme.fg("success", ` Selected: ${selectedLine.lineNum}#${selectedLine.hash}`));
		}
		add(this.theme.fg("dim", " j/k,↑/↓: navigate  |  g/G: first/last  |  enter: copy anchor  |  esc: close"));
		add(this.theme.fg("accent", "─".repeat(width)));

		this.cachedLines = lines;
		return lines;
	}

	invalidate(): void {
		this.cachedLines = undefined;
	}
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function parseHashreadOutput(output: string): {
	lines: HashlineDiffLine[];
	totalLines: number;
	truncated: boolean;
} {
	const lines: HashlineDiffLine[] = [];
	let totalLines = 0;
	let truncated = false;

	// Extract file content
	const fileMatch = output.match(/<file>\n([\s\S]*?)\n<\/file>/);
	if (!fileMatch) {
		return { lines, totalLines, truncated };
	}

	const fileContent = fileMatch[1].trim();
	const rawLines = fileContent.split("\n");

	for (const rawLine of rawLines) {
		// Format: LINE#HASH:content
		const match = rawLine.match(/^(\d+)#([A-Z]{2}):(.*)$/);
		if (match) {
			lines.push({
				sign: " ",
				lineNum: parseInt(match[1], 10),
				hash: match[2],
				content: match[3],
			});
		}
	}

	// Check for truncation message
	if (output.includes("(File has more lines")) {
		truncated = true;
		const truncMatch = output.match(/Use 'offset' parameter to read beyond line (\d+)/);
		if (truncMatch) {
			totalLines = parseInt(truncMatch[1], 10);
		}
	} else if (output.includes("(End of file")) {
		const endMatch = output.match(/(\d+) total lines/);
		if (endMatch) {
			totalLines = parseInt(endMatch[1], 10);
		} else {
			totalLines = lines.length;
		}
	} else {
		totalLines = lines.length;
	}

	return { lines, totalLines, truncated };
}

function parseEditOutput(output: string): {
	firstChangedLine: number | null;
	diffLines: HashlineDiffLine[];
} {
	const firstChangedLineMatch = output.match(/first change at line (\d+)/);
	const firstChangedLine = firstChangedLineMatch ? parseInt(firstChangedLineMatch[1], 10) : null;

	const diffLines: HashlineDiffLine[] = [];
	const diffMatch = output.match(/<diff>\n([\s\S]*?)\n<\/diff>/);
	if (!diffMatch) {
		return { firstChangedLine, diffLines };
	}

	const diffContent = diffMatch[1].trim();
	const rawLines = diffContent.split("\n");

	for (const rawLine of rawLines) {
		if (rawLine === "..." || rawLine.startsWith("---") || rawLine.startsWith("+++") || rawLine.startsWith("Note:")) {
			continue;
		}

		// Format: ±LINE#HASH:content or ±LINE#  :content (for deleted)
		const match = rawLine.match(/^([+-])(\d+)#([A-Z]{2}|  ):(.*)$/);
		if (match) {
			diffLines.push({
				sign: match[1] as "+" | "-",
				lineNum: parseInt(match[2], 10),
				hash: match[3] === "  " ? null : match[3],
				content: match[4],
			});
		}
	}

	return { firstChangedLine, diffLines };
}

// ─── Extension ────────────────────────────────────────────────────────────────

export default function HashlineTools(pi: ExtensionAPI) {
	// ─── hashread ────────────────────────────────────────────────────────────

	pi.registerTool({
		name: "hashread",
		label: "Hashread",
		description:
			"Read a file with hash anchors. Each line is prefixed with LINE#HASH: format where HASH is a 2-character " +
			"cumulative hash that changes if any previous line changes. Use these anchors with hashedit for reliable edits. " +
			"Output is truncated to 2000 lines or 50KB. Use 'offset' to read beyond the initial range.",
		promptSnippet: "Read file with hash anchors for reliable editing.",
		promptGuidelines: [
			"Use hashread before hashedit to get fresh hash anchors.",
			"After edits, lines after the edit region have stale hashes - use hashread to refresh.",
			"Copy LINE#HASH anchors from hashread output to use in hashedit operations.",
			"Use offset parameter to read beyond line 2000 in large files.",
		],
		parameters: Type.Object({
			path: Type.String({ description: "Path to file to read (relative or absolute)" }),
			offset: Type.Optional(Type.Number({ description: "Line offset to start reading from (0-indexed)", minimum: 0 })),
			limit: Type.Optional(Type.Number({ description: "Maximum lines to read (default: 2000)", minimum: 1, maximum: 2000 })),
		}),

		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			const normalizedPath = params.path.startsWith("@") ? params.path.slice(1) : params.path;
			const offset = params.offset ?? 0;
			const limit = params.limit ?? 2000;

			// Build command arguments
			const args = ["read", normalizedPath];
			if (offset > 0) args.push("--offset", String(offset));
			if (limit < 2000) args.push("--limit", String(limit));

			const result = await pi.exec("hashline-tools", args);

			if (result.code !== 0) {
				return {
					content: [{ type: "text", text: `hashread error: ${result.stderr}` }],
					details: { path: normalizedPath, lineCount: 0, offset, limit, truncated: false } as HashlineReadDetails,
					isError: true,
				};
			}

			const { lines, totalLines, truncated } = parseHashreadOutput(result.stdout);
			
			// Build content with hash anchors for the agent to see
			const anchorLines = lines.map((line) => `${line.lineNum}#${line.hash}:${line.content}`).join("\n");
			const contentText = `Read ${normalizedPath}: ${totalLines} lines\n\n${anchorLines}`;
			
			return {
				content: [{ type: "text", text: contentText }],
				details: {
					path: normalizedPath,
					lineCount: totalLines,
					offset,
					limit,
					truncated,
					rawOutput: result.stdout,
				} as HashlineReadDetails & { rawOutput: string },
			};
		},


		renderCall(args, theme) {
			const path = args.path;
			const offset = args.offset ?? 0;
			const limit = args.limit ?? 2000;
			let text = theme.fg("toolTitle", theme.bold("hashread "));
			text += theme.fg("text", path);
			if (offset > 0 || limit < 2000) {
				text += theme.fg("dim", ` [offset:${offset}, limit:${limit}]`);
			}
			return new Text(text, 0, 0);
		},

		renderResult(result, { expanded }, theme) {
			const details = result.details as HashlineReadDetails | undefined;
			if (!details) {
				const t = result.content[0];
				return new Text(t?.type === "text" ? t.text : "", 0, 0);
			}
			
			const lines = parseHashreadOutput((details as any).rawOutput || "").lines;
			
			// Build header
			const headerLines = [
				`${theme.fg("accent", "📄")} ${details.path}`,
				`${details.lineCount} lines${details.truncated ? theme.fg("warning", " (truncated)") : ""}`,
			];
			
			if (!expanded) {
				// Compact view: just show header with ellipsis if there's content
				if (lines.length > 0) {
					headerLines.push(theme.fg("dim", "  (expand to view content)"));
				}
			} else {
				// Expanded view: show all lines
				for (const line of lines) {
					headerLines.push(`  ${theme.fg("dim", `${line.lineNum}#${line.hash}`.padEnd(10))} ${theme.fg("text", line.content)}`);
				}
			}
			
			return new Text(headerLines.join("\n"), 0, 0);
		},
	});

	// ─── hashedit ────────────────────────────────────────────────────────────

	pi.registerTool({
		name: "hashedit",
		label: "Hashedit",
		description:
			"Edit a file using hash-aware operations. Supports replace, append, and prepend operations with LINE#HASH anchors. " +
			"Hashes are validated before applying edits - if hashes don't match, the operation fails with a helpful error showing " +
			"updated anchors. Returns a hash-aware diff with fresh hashes for edited lines. " +
			"Operations: replace (single line or range), append (after line or EOF), prepend (before line or BOF), delete. " +
			"The 'pos' and 'end' parameters are REQUIRED for all operations. Use 'EOF' for append at end of file, 'BOF' for prepend at start."
		,
		promptGuidelines: [
			"Always use hashread first to get fresh hash anchors before editing.",
			"Copy LINE#HASH anchors exactly from hashread output or previous hashedit diff.",
			"If hashedit fails with hash mismatch, use the updated anchors from the error message.",
			"Lines after edited regions have stale hashes - use hashread to refresh before further edits.",
			"Use 'replace' for modifications, 'append' to insert after, 'prepend' to insert before.",
			"Multiple edits can be applied atomically in a single hashedit call.",
		],
		parameters: Type.Object({
			path: Type.String({ description: "Path to file to edit (relative or absolute)" }),
			edits: Type.Array(
				Type.Union([
					// Replace operation - pos is required, end is optional for ranges
					Type.Object({
						op: Type.Literal("replace"),
						pos: Type.String({ description: 'Start anchor in "LINE#HASH" format (e.g., "8#RT"). REQUIRED.' }),
						end: Type.String({ description: 'Start anchor in "LINE#HASH" format (e.g., "8#RT"). REQUIRED.' }),
						lines: Type.Array(Type.String(), { description: "New line content ONLY (replaces matched lines). Do NOT include LINE#HASH: prefix - the tool computes new hashes automatically." })
					}),
					// Append operation - pos is required, use "EOF" to append at end
					Type.Object({
						op: Type.Literal("append"),
						pos: Type.String({ description: 'Anchor to append after in "LINE#HASH" format, or "EOF" to append at end of file. REQUIRED.' }),
						lines: Type.Array(Type.String(), { description: "Lines to append (content ONLY). Do NOT include LINE#HASH: prefix - the tool computes new hashes automatically." })
					}),
					// Prepend operation - pos is required, use "BOF" to prepend at start
					Type.Object({
						op: Type.Literal("prepend"),
						pos: Type.String({ description: 'Anchor to prepend before in "LINE#HASH" format, or "BOF" to prepend at start of file. REQUIRED.' }),
						lines: Type.Array(Type.String(), { description: "Lines to prepend (content ONLY). Do NOT include LINE#HASH: prefix - the tool computes new hashes automatically." })
					}),
					// Delete operation - pos is required, end is optional for ranges
					Type.Object({
						op: Type.Literal("delete"),
						pos: Type.String({ description: 'Start anchor in "LINE#HASH" format. REQUIRED.' }),
						end: Type.String({ description: 'Start anchor in "LINE#HASH" format. REQUIRED.' }),
					}),
				]),
				{ minItems: 1, description: "Array of edit operations to apply atomically. Each operation requires a 'pos' parameter." },
			),
		}),
		async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
			const normalizedPath = params.path.startsWith("@") ? params.path.slice(1) : params.path;

			// Process edits: handle delete, EOF, and BOF
			const processedEdits = params.edits.map((edit) => {
				if (edit.op === "delete") {
					return {
						op: "replace" as const,
						pos: edit.pos,
						...(edit.end && { end: edit.end }),
						lines: [],
					};
				}
				
				// Handle EOF/BOF - remove pos for these special cases
				if (edit.op === "append" && edit.pos === "EOF") {
					return {
						op: edit.op,
						lines: edit.lines,
					};
				}
				
				if (edit.op === "prepend" && edit.pos === "BOF") {
					return {
						op: edit.op,
						lines: edit.lines,
					};
				}
				
				return edit;
			});

			const editsJson = JSON.stringify(processedEdits);

			const result = await pi.exec("hashline-tools", ["edit", normalizedPath, "--edits", editsJson]);

			if (result.code !== 0) {
				return {
					content: [{ type: "text", text: `hashedit error: ${result.stderr || result.stdout}` }],
					details: { path: normalizedPath, firstChangedLine: null, operations: params.edits.length, success: false } as HashlineEditDetails,
					isError: true,
				};
			}

			const { firstChangedLine, diffLines } = parseEditOutput(result.stdout);
			const success = !result.stdout.includes("No changes made") && !result.stdout.includes("Hash mismatch");

			return {
				content: [{ type: "text", text: `Edited ${normalizedPath}: ${diffLines.length} lines changed` }],
				details: {
					path: normalizedPath,
					firstChangedLine,
					operations: params.edits.length,
					success,
					rawOutput: result.stdout,
				} as HashlineEditDetails & { rawOutput: string },
			};
		},

		renderCall(args, theme) {
			const path = args.path;
			const ops = args.edits.map((e: any) => e.op).join(", ");
			let text = theme.fg("toolTitle", theme.bold("hashedit "));
			text += theme.fg("text", path);
			text += theme.fg("dim", ` [${ops}]`);
			return new Text(text, 0, 0);
		},

		renderResult(result, { expanded }, theme) {
			const details = result.details as HashlineEditDetails | undefined;
			if (!details) {
				const t = result.content[0];
				return new Text(t?.type === "text" ? t.text : "", 0, 0);
			}

			if (!details.success) {
				return new Text(theme.fg("error", "✗ Edit failed or no changes made"), 0, 0);
			}

			const { diffLines } = parseEditOutput((details as any).rawOutput || "");

			const lines: string[] = [];
			lines.push(`${theme.fg("success", "✓")} Edited: ${details.path}`);
			if (details.firstChangedLine) {
				lines.push(theme.fg("dim", `  First change at line ${details.firstChangedLine}`));
			}
			lines.push("");

		// Show diff with colorful highlighting
		for (const line of diffLines) {
			let lineText = "";
			
			if (line.sign === "+") {
				// Added lines: green
				lineText += theme.fg("toolDiffAdded", "  +  ");
				lineText += theme.fg("dim", `${String(line.lineNum).padStart(4)}#${line.hash || "  "}`.padEnd(10));
				lineText += theme.fg("toolDiffAdded", line.content);
			} else if (line.sign === "-") {
				// Removed lines: red
				lineText += theme.fg("toolDiffRemoved", "  -  ");
				lineText += theme.fg("dim", `${String(line.lineNum).padStart(4)}#${line.hash || "  "}`.padEnd(10));
				lineText += theme.fg("toolDiffRemoved", line.content);
			} else {
				// Context lines: gray
				lineText += "      ";
				lineText += theme.fg("dim", `${String(line.lineNum).padStart(4)}#${line.hash || "  "}`.padEnd(10));
				lineText += theme.fg("toolDiffContext", line.content);
			}
			
			lines.push(lineText);
		}

			if (expanded) {
				lines.push("");
				lines.push(theme.fg("dim", "Note: Lines after edited regions have stale hashes. Use hashread to refresh."));
			}

			return new Text(lines.join("\n"), 0, 0);
		},
	});
}
