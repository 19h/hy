#!/bin/sh
printf 'script|%s\n' "$*" >> "$HY_TEST_EVENTS"
printf 'script output\n'
exit "${HY_TEST_CHILD_STATUS:-0}"
