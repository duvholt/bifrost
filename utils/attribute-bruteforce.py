#!/usr/bin/env python3

import sys
import time
import json

## Delay between each query, in seconds (adjust as needed)
##
## WARNING: Too small values might overload the zigbee network and/or the
## coordinator.
DELAY = 0.3

TMPL = {
    "topic":"DUMMY/set",
    "payload":{
        "read":{
            "cluster":64516,
            "attributes":[],
            "options":{
                "timeout": 500.0,
                "manufacturerCode":4107
            },
        }
    }
}

def note(msg):
    print(f">>> {msg} <<<", file=sys.stderr)

def main(argv0, *args):
    if len(args) != 1:
        print(f"usage: {argv0} <topic>")
        return 1

    topic = f"{args[0]}/set"
    TMPL["topic"] = topic
    note(f"Using topic {topic}")

    for cluster in [0xFC00, 0xFC01, 0xFC02, 0xFC03, 0xFC04]:
        note(f"Brute forcing attributes in cluster {cluster:04X}")
        TMPL["payload"]["read"]["cluster"] = cluster
        for x in range(64):
            TMPL["payload"]["read"]["attributes"] = [x]
            print(json.dumps(TMPL))
            sys.stdout.flush()
            time.sleep(DELAY)

    note("DONE")
    return 0

if __name__ == "__main__":
    sys.exit(main(*sys.argv))
