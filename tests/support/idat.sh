#!/bin/sh
for argument in "$@"; do
    case "$argument" in
        -L*) printf '__hcli__:%s\n' "$HY_TEST_IDA_PROBE" > "${argument#-L}" ;;
    esac
done
