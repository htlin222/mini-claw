# Playwright CLI for Pi Agent

Browser automation CLI tool. Use `pw` commands to control a headless Chromium browser.

## Quick Start

```bash
# Fetch page content
pw fetch example.com

# Fetch and take screenshot
pw fetch example.com -o /tmp/screenshot.png

# Full page screenshot
pw fetch example.com -o /tmp/full.png --full-page
```

## Command Reference

### fetch (Recommended)

The `fetch` command is the most useful - it navigates to a URL and extracts content or takes a screenshot in a single operation.

```bash
# Get page text content
pw fetch <url>

# Get page HTML
pw fetch <url> --format html

# Take screenshot
pw fetch <url> -o /path/to/screenshot.png

# Full page screenshot
pw fetch <url> -o /path/to/screenshot.png --full-page

# Explicit screenshot flag (same as -o)
pw fetch <url> --screenshot
```

### Navigation

```bash
pw navigate <url>   # Go to URL (auto-adds https://)
pw back             # Go back
pw forward          # Go forward
pw reload           # Reload page
```

### Screenshot

```bash
pw screenshot                      # Save to default location
pw screenshot -o /path/to/file.png # Save to specific path
pw screenshot --full-page          # Capture entire page
```

### Interaction

```bash
pw click <selector>           # Click element
pw type <selector> <text>     # Type text (appends)
pw fill <selector> <value>    # Fill input (replaces)
pw select <selector> <value>  # Select dropdown option
pw hover <selector>           # Hover over element
pw focus <selector>           # Focus element
pw press <key>                # Press key (Enter, Tab, Escape, etc.)
```

### Content Extraction

```bash
pw content                # Get page text
pw content --format html  # Get page HTML
pw text <selector>        # Get element text
pw snapshot               # Get accessibility tree
```

### Wait

```bash
pw wait-selector <selector>  # Wait for element
pw wait-text <text>          # Wait for text on page
pw wait-navigation           # Wait for navigation
pw wait <ms>                 # Wait milliseconds
```

### Session

```bash
pw status    # Check browser status
pw close     # Close browser
```

## Output Format

All commands return JSON:

```json
{
  "success": true,
  "url": "https://example.com",
  "title": "Example Domain",
  "content": "...",
  "timestamp": "2024-01-15T10:30:00.000Z"
}
```

Error responses include `error` field:

```json
{
  "success": false,
  "error": "Element not found",
  "timestamp": "2024-01-15T10:30:00.000Z"
}
```

## Selectors

Use CSS selectors or Playwright-specific selectors:

```bash
# CSS selectors
pw click "button.submit"
pw click "#login-form input[type=email]"

# Text selectors
pw click "text=Submit"

# Role selectors
pw click "role=button[name='Submit']"
```

## Examples

### Get page content

```bash
pw fetch news.ycombinator.com
# Returns: {"success": true, "content": "...", ...}
```

### Screenshot a website

```bash
pw fetch github.com -o /tmp/github.png
# Returns: {"success": true, "screenshot": "/tmp/github.png", ...}
```

### Fill and submit a form

```bash
pw navigate example.com/login
pw fill "input[name=email]" "user@example.com"
pw fill "input[name=password]" "secret123"
pw click "button[type=submit]"
```

### Extract specific element text

```bash
pw navigate example.com
pw text "h1"
# {"success": true, "text": "Example Domain", ...}
```

## Important Notes

1. **Each command is independent** - the browser closes after each command
2. **Use `fetch` for most tasks** - it combines navigate + content/screenshot
3. **Always check `success` field** in JSON output
4. **Screenshots are useful** for debugging
