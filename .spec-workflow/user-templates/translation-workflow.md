# Translation Workflow for AI Agents

## Purpose
Guide AI agents through the process of adding new message translations to NetHack Babel.

## Step 1: Extract New Message Keys
```bash
# Find all EngineEvent::msg() keys in the engine
grep -roh 'EngineEvent::msg("[^"]*")' crates/engine/src/ | sort -u > /tmp/engine_keys.txt

# Find all keys in the FTL file
grep -E '^[a-z].*=' data/locale/en/messages.ftl | sed 's/ =.*//' | sort > /tmp/ftl_keys.txt

# Find missing keys
comm -23 /tmp/engine_keys.txt /tmp/ftl_keys.txt
```

## Step 2: Find C Original Text
For each missing key, search the C source for the corresponding `pline()` call:
```bash
# Example: for key "spell-force-bolt"
grep -r "force bolt\|force_bolt" /Users/hz/Downloads/NetHack/src/*.c
```

## Step 3: Write Fluent Translation
Format rules:
- Simple: `message-id = The message text.`
- With variable: `message-id = { $name } hits { $target }!`
- Multiline (CRITICAL -- ALL continuation lines MUST be indented):
  ```
  message-id = { $count ->
      [one] one item
     *[other] { $count } items
  }
  ```
- Special chars: Use `{"{"}`  for literal `{`, `{"}"}` for literal `}`

## Step 4: Verify
```bash
cargo test -p nethack-babel-i18n 2>&1 | tail -5
```
All tests must pass. The Fluent parser is strict about indentation.

## Common Mistakes
1. Forgetting to indent continuation lines -> ParseError
2. Unbalanced `{` `}` in message text -> ParseError
3. Using `=` inside message text without escaping -> may split message
4. Missing space after `=` in definition -> ParseError
