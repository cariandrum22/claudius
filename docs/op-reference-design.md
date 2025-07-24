# 1Password Reference Parsing Design

## Current Problem

When op:// references are embedded in URLs, the parser cannot deterministically identify boundaries. For example:

```
https://gateway.ai.cloudflare.com/v1/op://Private/CLOUDFLARE AI Gateway/Account ID/op://Private/CLOUDFLARE AI Gateway/Gateway ID/anthropic
```

Issues:
1. Is `/anthropic` part of the op:// reference or the URL path?
2. How do we know where one op:// reference ends and another begins?
3. Spaces in vault/item names make parsing even more complex

## Current Implementation Issues

The current `extract_op_reference` function uses heuristics:
- Check for spaces to end the reference
- Look for lowercase-only path components
- Count segments

These heuristics are fragile and can fail in many cases.

## Proposed Solutions

### Option 1: Explicit Delimiters (Recommended)

Use a template syntax to make boundaries explicit:

```bash
# Current (ambiguous)
ANTHROPIC_BASE_URL=https://gateway.ai.cloudflare.com/v1/op://Private/CLOUDFLARE AI Gateway/Account ID/op://Private/CLOUDFLARE AI Gateway/Gateway ID/anthropic

# Proposed (clear boundaries)
ANTHROPIC_BASE_URL=https://gateway.ai.cloudflare.com/v1/{{op://Private/CLOUDFLARE AI Gateway/Account ID}}/{{op://Private/CLOUDFLARE AI Gateway/Gateway ID}}/anthropic
```

Advantages:
- Clear, unambiguous boundaries
- Works with any characters in vault/item names
- Easy to parse with simple regex or state machine
- Similar to existing template systems (Handlebars, Jinja)

Implementation:
```rust
// Match {{op://...}} patterns
let pattern = regex::Regex::new(r"\{\{(op://[^}]+)\}\}").unwrap();
```

### Option 2: URL Encoding

Require URL encoding for op:// references in URLs:

```bash
ANTHROPIC_BASE_URL=https://gateway.ai.cloudflare.com/v1/op%3A%2F%2FPrivate%2FCLOUDFLARE%20AI%20Gateway%2FAccount%20ID/op%3A%2F%2FPrivate%2FCLOUDFLARE%20AI%20Gateway%2FGateway%20ID/anthropic
```

Advantages:
- Standard URL encoding
- No ambiguity with URL paths

Disadvantages:
- Hard to read and write
- Easy to make mistakes

### Option 3: Structured Format

Define a strict format for op:// references:

```
op://<vault>/<item>/<field>
op://<vault>/<item>/<section>/<field>
```

And require escaping of forward slashes in names:

```bash
# If item name contains "/", escape it as "\/"
op://Private/CLOUDFLARE AI Gateway\/Production/Account ID
```

Advantages:
- Predictable structure
- Can parse deterministically

Disadvantages:
- Requires users to escape special characters
- Still ambiguous when embedded in URLs

### Option 4: Reference Resolution Pass

Keep op:// references separate and use variable substitution:

```bash
# Define references separately
OP_ACCOUNT_ID=op://Private/CLOUDFLARE AI Gateway/Account ID
OP_GATEWAY_ID=op://Private/CLOUDFLARE AI Gateway/Gateway ID

# Use variable references in URL
ANTHROPIC_BASE_URL=https://gateway.ai.cloudflare.com/v1/${OP_ACCOUNT_ID}/${OP_GATEWAY_ID}/anthropic
```

Advantages:
- No parsing ambiguity
- Leverages existing variable expansion
- More maintainable

Disadvantages:
- Requires two-step definition
- More verbose

## Recommendation

I recommend **Option 1: Explicit Delimiters** using `{{op://...}}` syntax because:

1. It's unambiguous and deterministic
2. It's familiar to users (similar to template engines)
3. It handles all edge cases (spaces, special characters, nested paths)
4. It's backward compatible (we can still support bare op:// references where unambiguous)

## Implementation Plan

1. Add support for `{{op://...}}` syntax while maintaining backward compatibility
2. Update documentation to recommend the new syntax for URLs
3. Add warnings for ambiguous bare op:// references in URLs
4. Eventually deprecate bare op:// references in URLs

## Examples

```bash
# Simple reference (backward compatible)
API_KEY=op://vault/item/api-key

# URL with references (new syntax required)
BASE_URL=https://api.example.com/v1/{{op://vault/account/id}}/{{op://vault/account/region}}/endpoint

# Mixed content
AUTH_HEADER=Bearer {{op://Private/API Tokens/Production Token}}

# Multiple references
CONNECTION_STRING=postgres://{{op://db/prod/user}}:{{op://db/prod/password}}@{{op://db/prod/host}}/{{op://db/prod/database}}
```
