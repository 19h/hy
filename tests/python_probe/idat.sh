#!/bin/sh
count=0
if [ -f "$HY_TEST_COUNTER" ]; then
    IFS= read -r count < "$HY_TEST_COUNTER"
fi
count=$((count + 1))
printf '%s\n' "$count" > "$HY_TEST_COUNTER"
/usr/bin/env > "$HY_TEST_SNAPSHOTS/env-$count"
pwd > "$HY_TEST_SNAPSHOTS/cwd-$count"
printf '%s\n' "$@" > "$HY_TEST_SNAPSHOTS/args-$count"
if [ -d "$IDAUSR" ]; then
    /bin/cp -R "$IDAUSR" "$HY_TEST_SNAPSHOTS/idausr-$count"
fi
if [ "$HY_TEST_READ_STDIN" = 1 ]; then
    IFS= read -r line
    printf '%s\n' "$line" > "$HY_TEST_SNAPSHOTS/stdin-$count"
fi
for argument in "$@"; do
    case "$argument" in
        -L*) log=${argument#-L} ;;
        -S*) /bin/cp "${argument#-S}" "$HY_TEST_SNAPSHOTS/script-$count.py" ;;
    esac
done
printf '__hcli__:{"stdout_is_not_the_result":true}\n'
case "$HY_TEST_MODE:$count" in
    missing:*|fallback-missing:1) ;;
    malformed:*) printf '__hcli__:invalid\n' > "$log" ;;
    invalid-model:*) printf '__hcli__:{}\n' > "$log" ;;
    fallback-log:1) printf 'broken startup\n' > "$log" ;;
    *) printf '__hcli__:%s\n' "$HY_TEST_IDA_PROBE" > "$log" ;;
esac
exit "${HY_TEST_IDAT_STATUS:-0}"
