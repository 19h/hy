#!/bin/sh
case "$1:$2" in
    '-c:'*)
        case "$2" in
            *record_candidates*)
                printf 'lookup\n' >> "$HY_TEST_EVENTS"
                printf '%s' "$HY_TEST_LOOKUP_OUTPUT"
                printf '%s' "$HY_TEST_LOOKUP_ERROR" >&2
                exit "${HY_TEST_LOOKUP_STATUS:-0}"
                ;;
            *'eps[0].load()()'*)
                printf 'entry-point\n' >> "$HY_TEST_EVENTS"
                shift 2
                ;;
            *) printf 'python\n' >> "$HY_TEST_EVENTS" ;;
        esac
        ;;
    *) printf 'python\n' >> "$HY_TEST_EVENTS" ;;
esac
printf '<%s>\n' "$@"
printf 'environment:%s|%s|%s|%s\n' "${PYTHONHOME-unset}" "${VIRTUAL_ENV-unset}" "${PYTHONUTF8-unset}" "$PATH"
if [ "$HY_TEST_READ_STDIN" = 1 ]; then
    IFS= read -r line
    printf 'stdin:%s\n' "$line"
fi
if [ -n "$HY_TEST_SIGNAL" ]; then
    kill -"$HY_TEST_SIGNAL" $$
fi
exit "${HY_TEST_CHILD_STATUS:-0}"
