import { tool } from "@opencode-ai/plugin";
import { $ } from "bun";

export default tool({
  description:
    "Read file with hashline format. Each line prefixed with LINE:HASH|",
  args: {
    filePath: tool.schema.string().describe("The path to the file to read"),
    offset: tool.schema.number().optional().describe("Line offset (0-based)"),
    limit: tool.schema
      .number()
      .optional()
      .describe("Number of lines to read (default 2000)"),
  },
  async execute(args, context) {
    const filepath = args.filePath.startsWith("/")
      ? args.filePath
      : `${context.directory}/${args.filePath}`;

    const offsetArg =
      args.offset !== undefined ? [`--offset`, String(args.offset)] : [];
    const limitArg =
      args.limit !== undefined ? [`--limit`, String(args.limit)] : [];

    const result =
      await $`hashline-tools read ${filepath} ${offsetArg} ${limitArg}`;
    const output = result.text();

    // Set metadata for TUI display
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
