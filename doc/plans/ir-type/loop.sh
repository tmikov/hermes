#!/bin/bash
# Ralph loop: runs Claude Code repeatedly, one task per iteration.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROMPT_FILE="$SCRIPT_DIR/prompt.md"

# Debug mode: single iteration in interactive TUI so you can watch it work
if [[ "${1:-}" == "--debug" ]]; then
    cd "$PROJECT_DIR"
    exec claude --dangerously-skip-permissions --effort max -p "$(cat "$PROMPT_FILE")" --verbose
fi

MAX_ITERATIONS="${1:-50}"
LOG_DIR="$SCRIPT_DIR/logs"

mkdir -p "$LOG_DIR"

echo "=== Ralph loop starting ==="
echo "Project: $PROJECT_DIR"
echo "Max iterations: $MAX_ITERATIONS"
echo "Logs: $LOG_DIR"
echo ""

for i in $(seq 1 "$MAX_ITERATIONS"); do
    TIMESTAMP=$(date '+%Y%m%d-%H%M%S')
    LOG_FILE="$LOG_DIR/$TIMESTAMP-iteration-$i.log"

    echo "--- Iteration $i/$MAX_ITERATIONS [$(date)] ---"

    # Run Claude with the prompt, tee output to log
    cd "$PROJECT_DIR"
    if ! claude -p "$(cat "$PROMPT_FILE")" \
        --dangerously-skip-permissions \
        --effort max \
        2>&1 | tee "$LOG_FILE"; then
        echo "Claude exited with error. Check $LOG_FILE"
        break
    fi

    # Check for stop signals in the output
    if grep -q "RALPH_DONE" "$LOG_FILE"; then
        echo "=== All tasks complete or no actionable tasks ==="
        break
    fi

    if grep -q "RALPH_BLOCKED" "$LOG_FILE"; then
        echo "=== Task blocked, stopping loop ==="
        break
    fi

    echo ""
done

echo "=== Ralph loop finished after $i iterations ==="
