import { tool } from "@opencode-ai/plugin";
import { $ } from "bun";

export default tool({
  description: 'Read file with hashline format. Returns lines prefixed with LINE#HASH where HASH is a 2-character code (e.g., "AB", "X3"). CRITICAL: You MUST call hashread BEFORE every hashedit - even if you just did a hashedit, you MUST call hashread again because cumulative hashes change after any edit. The hash of each line depends on all previous lines, so editing any line invalidates hashes for ALL subsequent lines. Always use the fresh hashes from the most recent hashread output for your next edit. Example: "5#AB: const x = 5;" - use anchor "5#AB" in hashedit. Use offset for large files (e.g., offset=370, limit=20).',

  args: {
    filePath: tool.schema.string().describe("The path to the file to read"),
    offset: tool.schema.number().optional().describe("0-based line number to start reading from. USE THIS when you know approximately where content is (e.g., from error messages or previous reads showing line numbers). This is more efficient than reading the entire file."),
    limit: tool.schema.number().optional().describe("Maximum lines to read. Default 2000. Use smaller values WITH offset to read specific sections (e.g., offset=100, limit=50 reads lines 101-150)."),
  },
  async execute(args, context) {
    const filepath = args.filePath.startsWith("/") 
      ? args.filePath 
      : `${context.directory}/${args.filePath}`;
    
    const offsetArg = args.offset !== undefined ? [`--offset`, String(args.offset)] : [];
    const limitArg = args.limit !== undefined ? [`--limit`, String(args.limit)] : [];
    
    const result = await $`hashline-tools read ${filepath} ${offsetArg} ${limitArg}`.quiet();
    const output = result.text();
    
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
