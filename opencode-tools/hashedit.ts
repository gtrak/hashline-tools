import { tool } from "@opencode-ai/plugin";
import { $ } from "bun";

export default tool({
  description: "Edit file using hash-anchored line references. Use hashread first to get valid LINE:HASH anchors.",
  args: {
    filePath: tool.schema.string().describe("The path to the file to edit"),
    edits: tool.schema.array(
      tool.schema.union([
        tool.schema.object({
          set_line: tool.schema.object({
            anchor: tool.schema.string().describe('Line reference "LINE:HASH"'),
            new_text: tool.schema.string().describe("Replacement content"),
          }),
        }),
        tool.schema.object({
          replace_lines: tool.schema.object({
            start_anchor: tool.schema.string().describe('Start line "LINE:HASH"'),
            end_anchor: tool.schema.string().describe('End line "LINE:HASH"'),
            new_text: tool.schema.string().describe("Replacement content"),
          }),
        }),
        tool.schema.object({
          insert_after: tool.schema.object({
            anchor: tool.schema.string().describe('Insert after this "LINE:HASH"'),
            text: tool.schema.string().describe("Content to insert"),
          }),
        }),
        tool.schema.object({
          replace: tool.schema.object({
            old_text: tool.schema.string().describe("Text to find"),
            new_text: tool.schema.string().describe("Replacement text"),
            all: tool.schema.boolean().optional(),
          }),
        }),
      ])
    ).describe("Array of edit operations"),
  },
  async execute(args, context) {
    const filepath = args.filePath.startsWith("/") 
      ? args.filePath 
      : `${context.directory}/${args.filePath}`;
    
    const editsJson = JSON.stringify(args.edits);
    
    const result = await $`hashline-tools edit ${filepath} --edits ${editsJson}`;
    const output = result.text();
    
    // Extract just the diff part for cleaner display
    const diffMatch = output.match(/<diff>([\s\S]*)<\/diff>/);
    const diffContent = diffMatch ? diffMatch[1].trim() : output;
    
    // Set metadata to help TUI display
    context.metadata({
      title: `Edit: ${args.filePath}`,
      metadata: {
        file: args.filePath,
        operations: args.edits.length,
        diff: diffContent
      }
    });
    
    // Return clean output without XML wrappers
    return `Edited ${args.filePath}:\n\n${diffContent}`;
  },
});
