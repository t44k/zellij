# Zellij MCP (Model Context Protocol)

This crate provides Model Context Protocol (MCP) server functionality for Zellij, enabling AI assistants and language models to interact with terminal sessions programmatically.

## Overview

The Zellij MCP implementation allows AI tools to:
- Read pane content (including scrollback history)
- Write text to panes and execute commands
- Manage panes (create, focus, close)
- Control tabs and sessions
- Query terminal state
- Launch and manage plugins

## Architecture

### Socket-Based Server (Primary Interface)

Each Zellij session runs an MCP server accessible via Unix domain socket at:
```
/tmp/zellij-{UID}/zellij-mcp-{session_name}.sock
```

The socket server uses **direct IPC with crossbeam channels** for efficient communication:
- **No temporary files** for pane content retrieval
- Returns structured `PaneContents` data with scrollback, viewport, and selection info
- 5-second timeout for operations
- Minimal latency

### CLI Fallback (Legacy Support)

When the socket server is unavailable, a CLI-based fallback is provided:
- Uses `zellij action` commands
- Limited to CLI capabilities (e.g., `dump-screen` requires pane focus)
- Uses temporary files as a last resort
- Primarily for backward compatibility

## API Design

### Pane Content Retrieval

The primary mechanism for reading pane content uses `GetPaneScrollback` IPC:

```rust
// Internal flow:
MCP Request → ScreenInstruction::GetPaneScrollback
           → pane.pane_contents()
           → PaneContents { viewport, lines_above, lines_below, selection }
           → Response via channel
```

**Key Features:**
- Works with any pane (focused or unfocused)
- Structured data format
- Optional full scrollback history
- Offset and line limit support for large outputs
- No filesystem I/O

### Available Operations

#### Pane Operations
- `read_pane` - Get pane content with optional scrollback
- `write_to_pane` - Send text to a pane
- `run_command_in_pane` - Execute command in pane (with newline)
- `focus_pane` - Change focus to specific pane
- `new_pane` - Create new pane
- `close_pane` - Close specific pane
- `list_panes` - Get all panes with metadata

#### Tab Operations
- `new_tab` - Create new tab
- `close_tab` - Close active tab
- `go_to_tab` - Switch to tab by 0-based index or name (Note: tab names display as "Tab #1", "Tab #2", etc. but indices are 0-based)
- `query_tab_names` - List all tabs with names and 0-based indices

#### Session Operations
- `rename_session` - Change session name
- `dump_layout` - Export current layout

#### Plugin Operations
- `launch_plugin` - Start a plugin
- `list_aliases` - Query available plugin aliases

## Usage Example

### From MCP Client

```json
{
  "operation": "read_pane",
  "args": {
    "pane_id": "terminal_1",
    "include_scrollback": true,
    "lines": 100,
    "offset": 0
  }
}
```

### Response Format

```json
{
  "content": "Pane terminal_1 content (lines 0-99 of 1500):\n<content here>"
}
```

## Integration with Zellij

The MCP server is automatically started when a Zellij session begins:

1. Server thread spawns in background
2. Unix socket created at known path
3. Listens for JSON-RPC requests
4. Routes commands to appropriate handlers
5. Returns responses via socket

### Server Lifecycle

- **Start:** Automatically with session
- **Location:** `/tmp/zellij-{UID}/zellij-mcp-{session_name}.sock`
- **Permissions:** User-only access (Unix socket permissions)
- **Cleanup:** Automatic on session exit

## Pane Identification

Panes are identified by type and ID:
- `terminal_{id}` - Terminal panes (e.g., `terminal_1`)
- `plugin_{id}` - Plugin panes (e.g., `plugin_2`)

Use `list_panes` to discover available panes and their IDs.

## Performance Characteristics

### Socket-Based (Recommended)
- **Latency:** <5ms for typical operations
- **Throughput:** Handles multiple concurrent requests
- **Overhead:** Minimal (in-memory channels)
- **Scalability:** Excellent for bulk operations

### CLI Fallback
- **Latency:** 50-100ms per operation (process spawn overhead)
- **Throughput:** Sequential only
- **Overhead:** Process creation + temp files
- **Scalability:** Poor for bulk operations

## Security Considerations

- Unix socket permissions restrict access to session owner
- Plugin permission system applies to all operations
- No network exposure (local socket only)
- No authentication required (OS-level security via file permissions)

## Error Handling

The MCP server provides structured error responses:

```json
{
  "error": "Pane terminal_999 not found"
}
```

Common errors:
- `Missing pane_id` - Required parameter not provided
- `Invalid pane ID` - Malformed pane identifier
- `Pane not found` - Pane no longer exists
- `Timeout waiting for pane contents` - Operation exceeded 5s limit

## Implementation Notes

### Why Direct IPC?

The original implementation used temporary files for pane content retrieval. This was replaced with direct IPC for several reasons:

1. **Performance:** Eliminates disk I/O and file operations
2. **Reliability:** No race conditions or cleanup issues
3. **Structure:** Returns rich data (viewport, scrollback, selection) instead of plain text
4. **Simplicity:** Fewer moving parts, easier to debug

### Relationship to `dump_pane_to_file`

The `dump_pane_to_file` functionality still exists for:
- **Edit scrollback feature:** Users editing terminal history in `$EDITOR` (requires file)
- **Manual export:** `zellij action dump-screen` CLI command
- **CLI fallback:** When socket server unavailable

However, **MCP does not use `dump_pane_to_file`** - it uses `GetPaneScrollback` IPC instead.

## Development

### Building

```bash
cargo build -p zellij-mcp
```

### Testing

```bash
# Unit tests
cargo test -p zellij-mcp

# Integration tests (requires running session)
cargo test -p zellij-server mcp
```

### Adding New Operations

1. Define operation in `tools/` module
2. Add handler in `mcp_server.rs`
3. Route request in dispatch logic
4. Update this README

## Future Enhancements

Potential improvements:
- [ ] WebSocket support for remote access
- [ ] Authentication/authorization layer
- [ ] Rate limiting for bulk operations
- [ ] Streaming responses for large outputs
- [ ] Event subscriptions (pane changes, new tabs, etc.)
- [ ] Binary protocol option (protobuf)

## See Also

- [Model Context Protocol Specification](https://spec.modelcontextprotocol.io/)
- [Zellij Plugin API](../zellij-tile/src/shim.rs)
- [Zellij Architecture](../docs/ARCHITECTURE.md)
