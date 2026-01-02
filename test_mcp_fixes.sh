#!/bin/bash

SOCKET="/home/devuser/.cache/zellij/test-mcp-fixes.mcp.sock"

# Function to send request to MCP server
send_mcp_request() {
    local operation=$1
    local args=$2
    echo "{\"operation\":\"$operation\",\"args\":$args}" | socat - UNIX-CONNECT:$SOCKET
}

echo "=== Testing MCP Fixes ==="
echo ""

# Test 1: get_session_info (PREVIOUSLY FAILED)
echo "Test 1: get_session_info"
echo "Note: This previously failed with 'No active zellij sessions found'"
# Note: get_session_info is not exposed via MCP server, it's in the client tools
echo "Skipping - this is a client-side function"
echo ""

# Test 2: list_panes (baseline test - should work)
echo "Test 2: list_panes (baseline)"
send_mcp_request "list_panes" "{}"
echo ""

# Test 3: launch_plugin with alias (PREVIOUSLY FAILED)
echo "Test 3: launch_plugin with alias 'strider' (PREVIOUSLY FAILED)"
send_mcp_request "launch_plugin" '{"url":"strider","floating":true}'
echo ""

# Test 4: Read a pane
echo "Test 4: read_pane"
send_mcp_request "read_pane" '{"pane_id":"terminal_0","lines":10}'
echo ""

# Test 5: Create a new pane
echo "Test 5: new_pane"
send_mcp_request "new_pane" '{"direction":"down"}'
sleep 1
echo ""

# Test 6: List panes to see the new one
echo "Test 6: list_panes (after creating new pane)"
send_mcp_request "list_panes" "{}"
echo ""

# Test 7: rename_session (PREVIOUSLY FAILED - broke MCP connection)
echo "Test 7: rename_session to 'renamed-test' (PREVIOUSLY FAILED)"
send_mcp_request "rename_session" '{"new_name":"renamed-test"}'
echo ""

# Wait for rename to complete
sleep 2

# Test 8: Check if MCP still works after rename
echo "Test 8: list_panes after rename (testing if MCP connection survived)"
SOCKET_RENAMED="/home/devuser/.cache/zellij/renamed-test.mcp.sock"
if [ -e "$SOCKET_RENAMED" ]; then
    echo "MCP socket renamed successfully!"
    echo "{\"operation\":\"list_panes\",\"args\":{}}" | socat - UNIX-CONNECT:$SOCKET_RENAMED
else
    echo "ERROR: MCP socket not found at $SOCKET_RENAMED"
    echo "Checking old socket location..."
    if [ -e "$SOCKET" ]; then
        echo "Old socket still exists (BUG NOT FIXED)"
    else
        echo "Old socket gone, but new socket not created (PARTIAL FIX)"
    fi
fi
echo ""

echo "=== Tests Complete ==="
