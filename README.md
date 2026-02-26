I was following [Pi](https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent) agent development, and intrigued by [oh-my-pi](https://github.com/can1357/oh-my-pi), but decided to start building on top of opencode instead.  
I am starting to regret that decision, but will see how far it goes.

[Vibe-kit Disclaimer](https://github.com/gtrak/vibe-kit/blob/main/DISCLAIMER.md)

I absolutely loved this blog post https://blog.can.ac/2026/02/12/the-harness-problem/ because I see failed edits constantly with Local LLMs, and this is a novel approach.  
Is it very novel though? BASIC had lines that start with indexes, too.  I want to use it, anyway. GOTO 200.

200 I am attempting to create a CLI version of this edit tool for later opencode integration, but it might stand alone. This was initially ported from oh-my-pi with LLM help 
into an opencode tool, then later extracted as a Rust CLI.

## Usage in OpenCode

Run `cargo install --path .` or create a wrapper script called 'hashline-tools' over cargo run.

Copy the typescript wrappers in [./opencode-tools](./opencode-tools) to ~/.config/opencode/tools or a specific project folder's .opencode.

## Hash-Aware Diff Format

After applying edits, the tool returns a custom diff format that includes freshly calculated hashes for immediate reuse:

```
Edit applied successfully (first change at line 8).

<diff>
--- /tmp/file.txt
+++ /tmp/file.txt
 3#NQ:line 3
 4#RH:line 4
 5#XH:line 5
 6#ZT:line 6
 7#BX:line 7
-8#  :line 8
+8#RT:modified line 8
 9#PJ:line 9
 10#NV:line 10

Note: Lines after edited regions have stale hashes. Use hashread to refresh.
</diff>
```

**Format:**
- `LINE#HASH:content` for context and inserted lines (fresh hashes)
- `LINE#  :content` for deleted lines (no hash, shown as 2 spaces)
- `±5` lines of context around each change
- Shows `...` for gaps between change regions
- Terse note: "Lines after edited regions have stale hashes. Use hashread to refresh."

This allows successive edits without an intermediate `hashread` call - just copy the `LINE#HASH` anchor from the diff output.

## API Format Changes from Original

This implementation diverges from oh-my-pi in several ways:

### Anchor Format

**Original:** `{line: 8, hash: "RT"}` (object)

**This implementation:** `"8#RT"` (string)

The string format aligns with:
- hashread output: `8#RT:line content`
- hashedit input: `"pos": "8#RT"`
- diff output: `+8#RT:modified content`

### Edit Operations

**Replace single line:**
```json
{
  "op": "replace",
  "pos": "8#RT",
  "lines": ["new content"]
}
```

**Replace range:**
```json
{
  "op": "replace",
  "pos": "6#ZT",
  "end": "10#NV",
  "lines": ["replaced content"]
}
```

**Append after line:**
```json
{
  "op": "append",
  "pos": "8#RT",
  "lines": ["new line after"]
}
```

**Append to EOF:**
```json
{
  "op": "append",
  "lines": ["new line at end"]
}
```

**Prepend before line:**
```json
{
  "op": "prepend",
  "pos": "8#RT",
  "lines": ["new line before"]
}
```

**Prepend to BOF:**
```json
{
  "op": "prepend",
  "lines": ["new line at start"]
}
```

**Delete single line:**
```json
{
  "op": "delete",
  "pos": "8#RT"
}
```

**Delete range:**
```json
{
  "op": "delete",
  "pos": "6#ZT",
  "end": "10#NV"
}
```

## Known Issues

- Diffs are not easily displayed in the opencode TUI due to external tool restrictions https://github.com/anomalyco/opencode/issues/6831#issuecomment-3910139894
