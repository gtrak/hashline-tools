import { tool } from "@opencode-ai/plugin";
import { $ } from "bun";

export default tool({
  description: `Read a file in hashline format. Each line is prefixed with a chained anchor in the form LINE#HASH (e.g. "5#AB: const x = 5;"). Use "5#AB" as the pos anchor in hashedit.

Hashes are chained: each line's hash depends on all preceding lines. This means:
- Anchors above an edit point remain valid after an edit.
- Anchors at or below an edit point are invalidated and must not be reused.
- hashedit returns fresh anchors for the affected region on both success and error — use those directly for follow-up edits without calling hashread again.

Call hashread when:
- Starting work on a file you haven't read yet.
- You need anchors for a region that wasn't covered by a recent hashedit response.
- You are uncertain whether your current anchors are still valid.

Use offset and limit to read specific regions of large files rather than the whole file.`,

  args: {
    filePath: tool.schema.string().describe("Path to the file to read"),
    offset: tool.schema
      .number()
      .describe(
        "0-based line number to start reading from. Use 0 to read from the beginning. " +
        "Use a higher value when you know approximately where the relevant content is " +
        "(e.g. from a line number in a previous error or diff)."
      ),
    limit: tool.schema
      .number()
      .describe(
        "Maximum number of lines to return. Use 2000 if you want to read the whole file. " +
        "Use a smaller value together with offset to read a specific region (e.g. offset=100, limit=50 returns lines 100–149)."
      ),
  },

  async execute(args, context) {
    const filepath = args.filePath.startsWith("/")
      ? args.filePath
      : `${context.directory}/${args.filePath}`;

    const result = await $`hashline-tools read ${filepath} --offset ${String(args.offset)} --limit ${String(args.limit)}`.quiet();
    const output = result.text();

    context.metadata({
      title: `Read: ${args.filePath}`,
      metadata: {
        file: args.filePath,
        offset: args.offset,
        limit: args.limit,
      },
    });

    return output;
  },
});
