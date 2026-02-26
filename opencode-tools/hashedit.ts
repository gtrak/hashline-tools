import { tool } from "@opencode-ai/plugin";
import { $ } from "bun";

function validateAnchor(anchor: any, fieldName: string): void {
  if (typeof anchor !== 'string') {
    throw new Error(`${fieldName}: must be a string in format "LINE#HASH" (e.g., "8#RT")`);
  }
  // Parse format: "LINE#HASH" (e.g., "8#RT")
  const parts = anchor.split('#');
  if (parts.length !== 2) {
    throw new Error(`${fieldName}: invalid format '${anchor}', expected "LINE#HASH" (e.g., "8#RT")`);
  }
  const lineNum = parseInt(parts[0], 10);
  if (isNaN(lineNum) || lineNum < 1) {
    throw new Error(`${fieldName}: line number must be a positive integer, got '${parts[0]}'`);
  }
  if (parts[1].length !== 2) {
    throw new Error(`${fieldName}: hash must be exactly 2 characters, got '${parts[1]}'`);
  }
}

function validateEdit(edit: any, op: string, index: number): void {
  if (!edit || typeof edit !== 'object') {
    throw new Error(`${op}[${index}]: must be an object`);
  }
  
  validateAnchor(edit.pos, `${op}[${index}].pos`);
  
  if (edit.end) {
    validateAnchor(edit.end, `${op}[${index}].end`);
  }
  
  if (op !== 'delete') {
    if (!Array.isArray(edit.lines)) {
      throw new Error(`${op}[${index}].lines: must be an array of strings`);
    }
    edit.lines.forEach((line: any, i: number) => {
      if (typeof line !== 'string') {
        throw new Error(`${op}[${index}].lines[${i}]: must be a string`);
      }
    });
  }
}

export default tool({
  description: "Edit file using hash-anchored line references. CRITICAL: hashread and hashedit must be called CONSECUTIVELY with NO OTHER COMMANDS in between. Do NOT run git, grep, sed, shell commands, or any other operations between hashread and hashedit - this will invalidate the hashes. If you restore a file with git, call hashread AGAIN afterward. When using multiple edits, combine them in ONE hashedit call with arrays, or re-hashread before each subsequent edit.",
  args: {
    filePath: tool.schema.string().describe("The path to the file to edit"),
    replace: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.string().describe('Anchor in format "LINE#HASH" from hashread output (e.g., "8#RT")'),
        end: tool.schema.optional(tool.schema.string().describe('End anchor for range replacement in format "LINE#HASH" (e.g., "10#BY")')),
        lines: tool.schema.array(tool.schema.string()).describe("Replacement lines"),
      })
    )).describe('Replace operations: [{pos: "LINE#HASH", lines: ["..."]}] or [{pos: "START#HASH", end: "END#HASH", lines: ["..."]}] for range'),
    append: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.optional(tool.schema.string().describe('Anchor in format "LINE#HASH" (e.g., "8#RT"). Omit for EOF.')),
        lines: tool.schema.array(tool.schema.string()),
      })
    )).describe('Append operations: [{pos: "LINE#HASH", lines: ["..."]}] inserts after specified line, or [{lines: ["..."]}] for EOF. TIP: To add a new method inside an impl block, append after the last method\'s closing brace (line with \'    }\' at 4-space indent, not the impl\'s \'}\' at 0-space indent).'),
    prepend: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.optional(tool.schema.string().describe('Anchor in format "LINE#HASH" (e.g., "8#RT"). Omit for BOF.')),
        lines: tool.schema.array(tool.schema.string()),
      })
    )).describe('Prepend operations: [{pos: "LINE#HASH", lines: ["..."]}] or [{lines: ["..."]}] for BOF'),
    delete: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.string().describe('Anchor in format "LINE#HASH" (e.g., "8#RT")'),
        end: tool.schema.optional(tool.schema.string().describe('End anchor for range deletion in format "LINE#HASH" (e.g., "10#BY")')),
      })
    )).describe('Delete operations: [{pos: "LINE#HASH"}] or [{pos: "START#HASH", end: "END#HASH"}] for range'),
    write: tool.schema.optional(tool.schema.string())
      .describe("Write operation: replace entire file with new content. Use this instead of replace/append/prepend/delete to completely rewrite the file."),
  },
  async execute(args, context) {
    const filepath = args.filePath.startsWith("/") 
      ? args.filePath 
      : `${context.directory}/${args.filePath}`;
    
    // Handle write operation - replaces entire file content
    if (args.write !== undefined) {
      const fileExists = await Bun.file(filepath).exists();
      await Bun.write(filepath, args.write);
      const stat = await Bun.file(filepath).stat();
      const size = stat?.size ?? args.write.length;
      
      context.metadata({
        title: `${fileExists ? 'Updated' : 'Created'}: ${args.filePath}`,
        metadata: {
          file: args.filePath,
          size: size,
          created: !fileExists,
        }
      });
      
      return `${fileExists ? 'Updated' : 'Created'} file: ${args.filePath}\nSize: ${size} bytes`;
    }
    
    const edits: any[] = [];
    
    if (args.replace) {
      args.replace.forEach((edit: any, i: number) => {
        validateEdit(edit, 'replace', i);
        edits.push({ op: 'replace', pos: edit.pos, end: edit.end, lines: edit.lines });
      });
    }
    
    if (args.append) {
      args.append.forEach((edit: any, i: number) => {
        validateEdit(edit, 'append', i);
        edits.push({ op: 'append', pos: edit.pos, lines: edit.lines });
      });
    }
    
    if (args.prepend) {
      args.prepend.forEach((edit: any, i: number) => {
        validateEdit(edit, 'prepend', i);
        edits.push({ op: 'prepend', pos: edit.pos, lines: edit.lines });
      });
    }
    
    if (args.delete) {
      args.delete.forEach((edit: any, i: number) => {
        validateEdit(edit, 'delete', i);
        // Delete is implemented as replace with empty lines
        edits.push({ op: 'replace', pos: edit.pos, end: edit.end, lines: [] });
      });
    }
    
    if (edits.length === 0) {
      throw new Error('No edits provided. Use at least one of: replace, append, prepend, delete, write.\n\nExample:\n  replace: [{pos: "8#RT", lines: ["new content"]}]\n  write: "entire file content here"');
    }
    
    const editsJson = JSON.stringify(edits);
    const editsResponse = new Response(editsJson);
    
    try {
      const result = await $`hashline-tools edit ${filepath} --edits-stdin < ${editsResponse}`.quiet();
      const output = result.text();
      
      const diffMatch = output.match(/<diff>([\s\S]*)<\/diff>/);
      const diffContent = diffMatch ? diffMatch[1].trim() : output;
      
      context.metadata({
        title: `Edit: ${args.filePath}`,
        metadata: {
          file: args.filePath,
          operations: edits.length,
          diff: diffContent
        }
      });
      
      return `Edited ${args.filePath}:\n\n${diffContent}`;
    } catch (error: any) {
      const stderr = error?.stderr?.toString() || error?.message || String(error);
      throw new Error(stderr);
    }
  },
});
