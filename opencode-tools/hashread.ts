import { tool } from "@opencode-ai/plugin";
import { $ } from "bun";
export default tool({
  description: "Read file with hashline format. Returns lines prefixed with LINE#HASH: where HASH is a 2-character code (e.g., 'AB', 'X3') that must be used in hashedit. Example: '5#AB: const x = 5;' - use pos: {line: 5, hash: 'AB'}. IMPORTANT: When you know the approximate line number (from error messages, stack traces, or previous reads), USE the 'offset' parameter instead of reading the entire file. For example, if looking for code around line 375, use offset=370 with limit=20. This is much more efficient than reading the whole file or making multiple small reads.",
  args: {
    filePath: tool.schema.string().describe("The path to the file to read"),
    offset: tool.schema.number().optional().describe("0-based line number to start reading from. USE THIS when you know approximately where content is (e.g., from error messages or previous reads showing line numbers). This is more efficient than reading the entire file."),
    limit: tool.schema.number().optional().describe("Maximum lines to read. Default 2000. Use smaller values WITH offset to read specific sections (e.g., offset=100, limit=50 reads lines 101-150). Avoid making multiple small reads of the same file - prefer one read with appropriate offset/limit."),
  },
  async execute(args, context) {
    const filepath = args.filePath.startsWith("/") 
      ? args.filePath 
      : `${context.directory}/${args.filePath}`;
    
    const offsetArg = args.offset !== undefined ? [`--offset`, String(args.offset)] : [];
    const limitArg = args.limit !== undefined ? [`--limit`, String(args.limit)] : [];
    
    const result = await $`hashline-tools read ${filepath} ${offsetArg} ${limitArg}`.quiet();
    const output = result.text();
    
    // Set metadata for TUI display
    context.metadata({
      title: `Read: ${args.filePath}`,
      metadata: {
        file: args.filePath,
        offset: args.offset,
        limit: args.limit
      }
    });
    
    return output;
  },
});