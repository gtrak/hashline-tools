import { tool } from "@opencode-ai/plugin";
import { $ } from "bun";

function validateEdit(edit: any, op: string, index: number): void {
  if (!edit || typeof edit !== 'object') {
    throw new Error(`${op}[${index}]: must be an object`);
  }
  
  if (edit.pos !== undefined && typeof edit.pos !== 'string') {
    throw new Error(`${op}[${index}].pos: must be a string in format "LINE#HASH" (e.g., "8#RT")`);
  }
  
  if (edit.end !== undefined && typeof edit.end !== 'string') {
    throw new Error(`${op}[${index}].end: must be a string in format "LINE#HASH" (e.g., "8#RT")`);
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
  description: "Edit file using hash-anchored line references. CRITICAL: hashread MUST be called immediately before every hashedit. Use anchors from the most recent hashread output in format LINE#HASH (e.g., \"8#RT\"). After editing, hashes for ALL lines after the edit point change. Combine multiple edits in one call when possible.",
  args: {
    filePath: tool.schema.string().describe("The path to the file to edit"),
    replace: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.string().describe('Anchor in format "LINE#HASH" from hashread (e.g., "8#RT")'),
        end: tool.schema.optional(tool.schema.string().describe('End anchor "LINE#HASH" for range replacement')),
        lines: tool.schema.array(tool.schema.string()).describe("Replacement lines"),
      })
    )).describe('Replace: [{pos: "LINE#HASH", lines: ["..."]}] or range [{pos: "START#HASH", end: "END#HASH", lines: ["..."]}]'),
    append: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.optional(tool.schema.string().describe('Anchor "LINE#HASH". Omit for EOF.')),
        lines: tool.schema.array(tool.schema.string()),
      })
    )).describe('Append: [{pos: "LINE#HASH", lines: ["..."]}] after line, or [{lines: ["..."]}] for EOF'),
    prepend: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.optional(tool.schema.string().describe('Anchor "LINE#HASH". Omit for BOF.')),
        lines: tool.schema.array(tool.schema.string()),
      })
    )).describe('Prepend: [{pos: "LINE#HASH", lines: ["..."]}] before line, or [{lines: ["..."]}] for BOF'),
    delete: tool.schema.optional(tool.schema.array(
      tool.schema.object({
        pos: tool.schema.string().describe('Anchor "LINE#HASH"'),
        end: tool.schema.optional(tool.schema.string().describe('End anchor for range deletion')),
      })
    )).describe('Delete: [{pos: "LINE#HASH"}] or range [{pos: "START#HASH", end: "END#HASH"}]'),
    write: tool.schema.optional(tool.schema.string())
      .describe("Write: replace entire file with new content."),
  },
  async execute(args, context) {
    const filepath = args.filePath.startsWith("/") 
      ? args.filePath 
      : `${context.directory}/${args.filePath}`;
    
    if (args.write !== undefined) {
      const fileExists = await Bun.file(filepath).exists();
      await Bun.write(filepath, args.write);
      const stat = await Bun.file(filepath).stat();
      const size = stat?.size ?? args.write.length;
      
      context.metadata({
        title: `${fileExists ? 'Updated' : 'Created'}: ${args.filePath}`,
        metadata: { file: args.filePath, size, created: !fileExists }
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
        edits.push({ op: 'replace', pos: edit.pos, end: edit.end, lines: [] });
      });
    }
    
    if (edits.length === 0) {
      throw new Error('No edits provided. Use replace/append/prepend/delete/write.');
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
        metadata: { file: args.filePath, operations: edits.length, diff: diffContent }
      });
      
      return `Edited ${args.filePath}:\n\n${diffContent}`;
    } catch (error: any) {
      const stderr = error?.stderr?.toString() || error?.message || String(error);
      throw new Error(stderr);
    }
  },
});
