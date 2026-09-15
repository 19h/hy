#!/bin/sh
printf '%s\n' "$@" > "$HY_TEST_INSTALLER_ARGUMENTS"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix) shift; prefix=$1 ;;
    esac
    shift
done
if [ -f "$HY_TEST_INSTALLER_ARGUMENTS.fail" ]; then
    printf '%s\n' 'fixture installer failure' >&2
    exit 1
fi
if [ -f "$HY_TEST_INSTALLER_ARGUMENTS.empty" ]; then
    exit 0
fi
if [ "$(uname -s)" = Darwin ]; then
    directory="$prefix/IDA Fixture.app/Contents/MacOS"
else
    directory=$prefix
fi
mkdir -p "$directory"
printf '%s\n' 'fixture documentation' > "$directory/ida.hlp"
printf '%s\n' 'fixture binary' > "$directory/ida"
