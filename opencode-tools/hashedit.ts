import { tool } from "@opencode-ai/plugin";
import { $ } from "bun";

function validateAnchor(anchor: any, fieldName: string): void {
  if (!anchor || typeof anchor !== 'object') {
    throw new Error(`${fieldName}: must be an object {line: number, hash: string}`);
  }
  if (typeof anchor.line !== 'number' || anchor.line < 1) {
    throw new Error(`${fieldName}.line: must be a positive number`);
  }
  if (typeof anchor.hash !== 'string' || anchor.hash.length !== 2) {
    throw new Error(`${fieldName}.hash: must be a 2-character string (e.g., 'AB')`);
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
  description: "Edit file using hash-anchored line references. CRITICAL: Must call hashread first to get valid LINE#HASH anchors.",
  args: {
    filePath: tool.schema.string().describe("The path to the file to edit"),
    replace: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.object({
          line: tool.schema.number().describe("Line number (1-based)"),
          hash: tool.schema.string().describe("Line hash (2 chars from LINE#HASH)"),
        }),
        end: tool.schema.optional(tool.schema.object({
          line: tool.schema.number(),
          hash: tool.schema.string(),
        })),
        lines: tool.schema.array(tool.schema.string()).describe("Replacement lines"),
      })
    )).describe("Replace operations: [{pos: {line, hash}, lines: ['...']}]"),
    append: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.optional(tool.schema.object({
          line: tool.schema.number(),
          hash: tool.schema.string(),
        })),
        lines: tool.schema.array(tool.schema.string()),
      })
    )).describe("Append operations: [{pos: {line, hash}, lines: ['...']}] or [{lines: ['...']}] for EOF"),
    prepend: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.optional(tool.schema.object({
          line: tool.schema.number(),
          hash: tool.schema.string(),
        })),
        lines: tool.schema.array(tool.schema.string()),
      })
    )).describe("Prepend operations: [{pos: {line, hash}, lines: ['...']}] or [{lines: ['...']}] for BOF"),
    delete: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.object({
          line: tool.schema.number(),
          hash: tool.schema.string(),
        }),
        end: tool.schema.optional(tool.schema.object({
          line: tool.schema.number(),
          hash: tool.schema.string(),
        })),
      })
    )).describe("Delete operations: [{pos: {line, hash}}] or [{pos, end}] for range"),
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
        edits.push({ op: 'delete', pos: edit.pos, end: edit.end });
      });
    }
    
    if (edits.length === 0) {
      throw new Error("No edits provided. Use at least one of: replace, append, prepend, delete, write.\n\nExample:\n  replace: [{pos: {line: 1, hash: 'AB'}, lines: ['new content']}]\n  write: 'entire file content here'");
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

      if (stderr.includes("lines have changed") || stderr.includes("HashlineMismatchError")) {
        const changedLinesMatch = stderr.match(/>>>\s*(\d+)#([A-Z]+):/g);
        throw new Error(`Hash mismatch: file was modified since last read. Re-read the file with hashread to get fresh LINE#HASH anchors, then retry the edit. The changed lines are: ${changedLinesMatch?.join(", ") || "see error details"}`);
      }

      throw error;
    }
  },
});
