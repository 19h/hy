#!/bin/sh
case "$1:$2" in
    '-c:import pip') exit 0 ;;
    '-c:'*) printf '3.13\n'; exit 0 ;;
    '-m:pip')
        case "$*" in
            *firstdep*) /bin/rm "$0" ;;
        esac
        exit 0
        ;;
    *) exit 1 ;;
esac
