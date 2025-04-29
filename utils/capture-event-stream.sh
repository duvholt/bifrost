#!/bin/sh

set -ue

if [ $# -ne 2 ]; then
    echo "usage: $0 <hue-key> <bridge-addr>"
    exit 1
fi

KEY="${1}"
IP="${2}"

exec curl -N --http2 \
     -H "Accept:text/event-stream" \
     -s \
     -k \
     -H"hue-application-key: ${KEY}" \
     "https://${IP}/eventstream/clip/v2"
