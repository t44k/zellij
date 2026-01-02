#!/bin/bash

SOCKET="/home/devuser/.cache/zellij/renamed-test.mcp.sock"

send_mcp_request() {
    local operation=$1
    local args=$2
    echo "{\"operation\":\"$operation\",\"args\":$args}" | socat - UNIX-CONNECT:$SOCKET 2>/dev/null
}

echo "=========================================="
echo "   EDGE CASE & STRESS TESTS"
echo "=========================================="
echo ""

echo "Test 1: Launch plugin with another alias (tab-bar)"
result=$(send_mcp_request "launch_plugin" '{"url":"tab-bar","floating":false}')
echo "Result: $result"
if echo "$result" | grep -q "success.*true"; then
    echo "✅ PASS - Can launch multiple different plugins by alias"
else
    echo "❌ FAIL"
fi
echo ""

echo "Test 2: Launch plugin with full URL (not alias)"
result=$(send_mcp_request "launch_plugin" '{"url":"file:/workspace/target/wasm32-wasip1/debug/strider.wasm","floating":true}')
echo "Result: $result"
if echo "$result" | grep -q "success.*true"; then
    echo "✅ PASS - Can still launch plugins with full URLs"
else
    echo "❌ FAIL"
fi
echo ""

echo "Test 3: Multiple renames in sequence"
echo "Rename to 'test-a'..."
result=$(send_mcp_request "rename_session" '{"new_name":"test-a"}')
echo "Result: $result"
sleep 2

SOCKET_A="/home/devuser/.cache/zellij/test-a.mcp.sock"
if [ -e "$SOCKET_A" ]; then
    echo "✅ First rename successful, socket exists at $SOCKET_A"
    
    echo "Rename again to 'test-b'..."
    result=$(echo '{"operation":"rename_session","args":{"new_name":"test-b"}}' | socat - UNIX-CONNECT:$SOCKET_A 2>/dev/null)
    echo "Result: $result"
    sleep 2
    
    SOCKET_B="/home/devuser/.cache/zellij/test-b.mcp.sock"
    if [ -e "$SOCKET_B" ] && [ ! -e "$SOCKET_A" ]; then
        echo "✅ PASS - Multiple renames work correctly"
        echo "   Old socket cleaned up: $([ ! -e "$SOCKET_A" ] && echo "YES" || echo "NO")"
        echo "   New socket exists: $([ -e "$SOCKET_B" ] && echo "YES" || echo "NO")"
        
        # Test if MCP still works
        result=$(echo '{"operation":"list_panes","args":{}}' | socat - UNIX-CONNECT:$SOCKET_B 2>/dev/null)
        if echo "$result" | grep -q "success.*true"; then
            echo "   MCP still functional: ✅ YES"
        else
            echo "   MCP still functional: ❌ NO"
        fi
    else
        echo "❌ FAIL - Second rename didn't work properly"
    fi
else
    echo "❌ FAIL - First rename failed"
fi
echo ""

echo "Test 4: Read pane with scrollback"
SOCKET_FINAL="/home/devuser/.cache/zellij/test-b.mcp.sock"
echo "Writing multiple lines to test scrollback..."
for i in {1..5}; do
    echo '{"operation":"write_to_pane","args":{"pane_id":"terminal_0","text":"Line '$i' for scrollback test","submit":true}}' | socat - UNIX-CONNECT:$SOCKET_FINAL 2>/dev/null > /dev/null
    sleep 0.2
done

result=$(echo '{"operation":"read_pane","args":{"pane_id":"terminal_0","lines":50,"include_scrollback":true}}' | socat - UNIX-CONNECT:$SOCKET_FINAL 2>/dev/null)
if echo "$result" | grep -q "Line 1"; then
    echo "✅ PASS - Can read pane with scrollback"
else
    echo "❌ FAIL - Scrollback not working"
    echo "Result: $result"
fi
echo ""

echo "Test 5: Close and recreate panes"
echo "Creating new pane..."
echo '{"operation":"new_pane","args":{"direction":"right"}}' | socat - UNIX-CONNECT:$SOCKET_FINAL 2>/dev/null > /dev/null
sleep 1

result=$(echo '{"operation":"list_panes","args":{}}' | socat - UNIX-CONNECT:$SOCKET_FINAL 2>/dev/null)
pane_count=$(echo "$result" | grep -o "terminal_" | wc -l)
echo "Pane count: $pane_count"

if [ "$pane_count" -gt 1 ]; then
    echo "✅ PASS - Multiple panes exist"
    
    # Close the new pane (terminal_1 or similar)
    terminal_id=$(echo "$result" | grep -o '"id":"terminal_[0-9]*"' | grep -v "terminal_0" | head -1 | cut -d'"' -f4)
    if [ -n "$terminal_id" ]; then
        echo "Closing pane: $terminal_id"
        echo '{"operation":"close_pane","args":{"pane_id":"'$terminal_id'"}}' | socat - UNIX-CONNECT:$SOCKET_FINAL 2>/dev/null > /dev/null
        sleep 1
        
        result=$(echo '{"operation":"list_panes","args":{}}' | socat - UNIX-CONNECT:$SOCKET_FINAL 2>/dev/null)
        new_pane_count=$(echo "$result" | grep -o "terminal_" | wc -l)
        
        if [ "$new_pane_count" -lt "$pane_count" ]; then
            echo "✅ PASS - Pane closed successfully"
        else
            echo "❌ FAIL - Pane not closed"
        fi
    fi
else
    echo "❌ FAIL - Failed to create additional pane"
fi
echo ""

echo "=========================================="
echo "   EDGE CASE TEST SUMMARY"
echo "=========================================="
echo ""
echo "All edge cases tested:"
echo "  • Multiple plugin types (aliases & URLs)"
echo "  • Sequential session renames"  
echo "  • Scrollback functionality"
echo "  • Pane lifecycle (create/close)"
echo ""
echo "Final socket location: /home/devuser/.cache/zellij/test-b.mcp.sock"
echo ""
