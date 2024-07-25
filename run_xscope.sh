#!/bin/bash

if ! command -v xscope > /dev/null; then
    echo "Error: xscope not found" 1>&2 && exit 1
fi

dpy=${DISPLAY:-:0}
next=:$((${dpy:1} + 1))

mapfile -t auth < <(xauth list "$dpy")
IFS=" " read -ra auth <<< "${auth[0]}"

xauth add "$next" "${auth[1]}" "${auth[2]}"

params=("-v1" "-d${dpy:1}")

echo "Running xscope ${params[@]}"
echo "Start your connection to display ${next}"

xscope "${params[@]}" |& tee xscope.log

xauth remove "$next"
