#!/bin/sh
if [ "$HY_TEST_FAIL_FIRST" = 1 ] && [ ! -f "$HY_TEST_EVENTS" ]; then
    echo idat >> "$HY_TEST_EVENTS"
    exit 1
fi
echo idat >> "$HY_TEST_EVENTS"
for argument in "$@"; do
    case "$argument" in
        -L*) printf '__hcli__:%s\n' "$HY_TEST_IDA_PROBE" > "${argument#-L}" ;;
    esac
done
