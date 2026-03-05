import { tool } from "@opencode-ai/plugin";
import { $ } from "bun";

type HashAnchor = string;
type PosAnchor = HashAnchor | "EOF" | "BOF";

/**
 * Validate an anchor value and return a helpful error message.
 * This is the single source of truth for anchor validation.
 */
function validateAnchor(
  value: unknown,
  field: string,
  options: {
    allowEof?: boolean;
    allowBof?: boolean;
    requireHashread?: boolean;
  } = {}
): string {
  const { allowEof = false, allowBof = false, requireHashread = true } = options;

  // Check type first
  if (typeof value !== "string") {
    throw new Error(
      `${field}: must be a string LINE#HASH (e.g., "8#RT"), "EOF", or "BOF" - ` +
        `got: ${JSON.stringify(value)} (${typeof value}). ` +
        `Hint: Make sure you call hashread first to get valid LINE#HASH anchors.`
    );
  }

  // Check for special values
  if (value === "EOF") {
    if (!allowEof) {
      throw new Error(
        `${field}: "EOF" is not allowed here. Use "append" with "EOF" to append at end of file.`
      );
    }
    return value;
  }

  if (value === "BOF") {
    if (!allowBof) {
      throw new Error(
        `${field}: "BOF" is not allowed here. Use "prepend" with "BOF" to prepend at start of file.`
      );
    }
    return value;
  }

  // Check format
  if (!/^\d+#[A-Za-z0-9]+$/.test(value)) {
    // Common mistake: passing just a number
    if (/^\d+$/.test(value)) {
      throw new Error(
        `${field}: got "${value}" which looks like a line number without a hash. ` +
          `You must use LINE#HASH format (e.g., "${value}#AB") from hashread output. ` +
          `Did you call hashread first to get valid anchors?`
      );
    }

    throw new Error(
      `${field}: must be LINE#HASH format (e.g., "8#RT"), "EOF", or "BOF" - ` +
        `got: ${JSON.stringify(value)}. ` +
        (requireHashread ? `Hint: Call hashread first to get valid LINE#HASH anchors.` : "")
    );
  }

  return value;
}

function validateLines(lines: unknown, field: string): string[] {
  if (!Array.isArray(lines)) {
    throw new Error(`${field}: must be an array of strings`);
  }
  lines.forEach((line: unknown, i: number) => {
    if (typeof line !== "string") {
      throw new Error(`${field}[${i}]: must be a string, got: ${typeof line}`);
    }
  });
  return lines as string[];
}

type ReplaceOp = { op: "replace"; pos: HashAnchor; end?: HashAnchor; lines: string[] };
type AppendOp  = { op: "append";  pos: PosAnchor; lines: string[] };
type PrependOp = { op: "prepend"; pos: PosAnchor; lines: string[] };
type DeleteOp  = { op: "delete";  pos: HashAnchor; end?: HashAnchor };
type WriteOp   = { op: "write";   content: string };
type EditOp    = ReplaceOp | AppendOp | PrependOp | DeleteOp | WriteOp;
type InternalOp = {
  op: "replace" | "append" | "prepend";
  pos?: string;
  end?: string;
  lines: string[];
};

function validateOp(raw: unknown, index: number): EditOp {
  const ctx = `edits[${index}]`;
  if (!raw || typeof raw !== "object") throw new Error(`${ctx}: must be an object`);
  
  const rawObj = raw as Record<string, unknown>;
  const { op } = rawObj;
  
  if (!op || typeof op !== "string") {
    throw new Error(`${ctx}: missing required field "op" - must be one of: replace, append, prepend, delete, write`);
  }

  switch (op) {
    case "write": {
      if (typeof rawObj.content !== "string") throw new Error(`${ctx}.content: must be a string`);
      return { op: "write", content: rawObj.content };
    }
    case "replace": {
      const pos = validateAnchor(rawObj.pos, `${ctx}.pos`, { allowEof: false, allowBof: false });
      let end: string | undefined;
      if (rawObj.end !== undefined) {
        end = validateAnchor(rawObj.end, `${ctx}.end`, { allowEof: false, allowBof: false });
      }
      const lines = validateLines(rawObj.lines, `${ctx}.lines`);
      return { op: "replace", pos, end, lines };
    }
    case "append": {
      const pos = validateAnchor(rawObj.pos, `${ctx}.pos`, { allowEof: true, allowBof: false });
      const lines = validateLines(rawObj.lines, `${ctx}.lines`);
      return { op: "append", pos, lines };
    }
    case "prepend": {
      const pos = validateAnchor(rawObj.pos, `${ctx}.pos`, { allowEof: false, allowBof: true });
      const lines = validateLines(rawObj.lines, `${ctx}.lines`);
      return { op: "prepend", pos, lines };
    }
    case "delete": {
      const pos = validateAnchor(rawObj.pos, `${ctx}.pos`, { allowEof: false, allowBof: false });
      let end: string | undefined;
      if (rawObj.end !== undefined) {
        end = validateAnchor(rawObj.end, `${ctx}.end`, { allowEof: false, allowBof: false });
      }
      return { op: "delete", pos, end };
    }
    default:
      throw new Error(`${ctx}.op: unknown operation "${op}" - must be one of: replace, append, prepend, delete, write`);
  }
}

function toInternalOp(edit: Exclude<EditOp, WriteOp>): InternalOp {
  switch (edit.op) {
    case "replace":
      return { op: "replace", pos: edit.pos, ...(edit.end ? { end: edit.end } : {}), lines: edit.lines };
    case "append":
      return edit.pos === "EOF"
        ? { op: "append", lines: edit.lines }
        : { op: "append", pos: edit.pos, lines: edit.lines };
    case "prepend":
      return edit.pos === "BOF"
        ? { op: "prepend", lines: edit.lines }
        : { op: "prepend", pos: edit.pos, lines: edit.lines };
    case "delete":
      return { op: "replace", pos: edit.pos, ...(edit.end ? { end: edit.end } : {}), lines: [] };
  }
}

export default tool({
  description: `Edit a file using hash-anchored line references.

Hashes are chained: each line's hash depends on all preceding lines.
- Anchors above an edit point remain valid after that edit.
- Anchors at or below an edit point are invalidated.
- Multiple edits in one call are applied bottom-to-top, so all anchors in the batch stay valid as long as they came from the same hashread or hashedit response.
- Fresh anchors for the affected region are returned on both success and error - use them directly for follow-up edits without calling hashread again.

Operations:
  "replace"  Replace one line or range (pos to end) with new lines.
  "append"   Insert lines after pos. Use "EOF" to append at end of file.
  "prepend"  Insert lines before pos. Use "BOF" to prepend at start of file.
  "delete"   Delete one line or range (pos to end).
  "write"    Replace entire file content. Does not use anchors. Cannot be combined with other ops.

Rules:
  pos/end must be LINE#HASH anchors (e.g. "8#RT") from the most recent hashread or hashedit response.`,

  args: {
    filePath: tool.schema.string().describe("Path to the file to edit"),
    edits: tool.schema.array(
      tool.schema.object({
        op: tool.schema
          .string()
          .describe('"replace" | "append" | "prepend" | "delete" | "write"'),
        pos: tool.schema
          .optional(tool.schema.string())
          .describe('LINE#HASH anchor from hashread output (e.g., "8#RT"). "append" also accepts "EOF"; "prepend" also accepts "BOF". Required for all ops except "write". NOTE: You MUST call hashread first to get valid LINE#HASH anchors.'),
        end: tool.schema
          .optional(tool.schema.string())
          .describe('End anchor LINE#HASH for range "replace" or "delete". Must also come from hashread output.'),
        lines: tool.schema
          .optional(tool.schema.array(tool.schema.string()))
          .describe('Lines to insert or replace. Required for "replace", "append", "prepend".'),
        content: tool.schema
          .optional(tool.schema.string())
          .describe('Full file content. Required for "write" only.'),
      })
    ).describe("One or more edit operations. Multiple ops are applied bottom-to-top automatically. You MUST call hashread first to get valid LINE#HASH anchors before using hashedit."),
  },

  async execute(args, context) {
    const filepath = args.filePath.startsWith("/")
      ? args.filePath
      : `${context.directory}/${args.filePath}`;

    if (!Array.isArray(args.edits) || args.edits.length === 0) {
      throw new Error('"edits" must be a non-empty array');
    }

    const validated = args.edits.map((raw: any, i: number) => validateOp(raw, i));

    const writeOps = validated.filter((e) => e.op === "write");
    if (writeOps.length > 1) throw new Error('Only one "write" op is allowed per call');
    if (writeOps.length === 1 && validated.length > 1) throw new Error('"write" cannot be combined with other ops');

    // --- write ---
    if (writeOps.length === 1) {
      const edit = writeOps[0] as WriteOp;
      const fileExists = await Bun.file(filepath).exists();
      await Bun.write(filepath, edit.content);
      const stat = await Bun.file(filepath).stat();
      const size = stat?.size ?? edit.content.length;
      context.metadata({
        title: `${fileExists ? "Updated" : "Created"}: ${args.filePath}`,
        metadata: { file: args.filePath, size, created: !fileExists },
      });
      return `${fileExists ? "Updated" : "Created"} file: ${args.filePath}\nSize: ${size} bytes`;
    }

    // --- structural ops ---
    const internalOps = (validated as Exclude<EditOp, WriteOp>[]).map(toInternalOp);
    const editsResponse = new Response(JSON.stringify(internalOps));

    try {
      const result = await $`hashline-tools edit ${filepath} --edits-stdin < ${editsResponse}`.quiet();
      const output = result.text();
      const diffMatch = output.match(/<diff>([\s\S]*)<\/diff>/);
      const diffContent = diffMatch ? diffMatch[1].trim() : output;

      context.metadata({
        title: `Edit: ${args.filePath}`,
        metadata: { file: args.filePath, ops: validated.length, diff: diffContent },
      });

      return `Edited ${args.filePath} [${validated.map((e) => e.op).join(", ")}]:\n\n${diffContent}`;
    } catch (error: any) {
      const stderr = error?.stderr?.toString() || error?.message || String(error);
      throw new Error(stderr);
    }
  },
});
