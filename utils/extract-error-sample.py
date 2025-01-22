#!/usr/bin/env python3

import sys
import re
import enum
import json

RE_LOG = re.compile("\s*[0-9]{4}-[0-9]{2}-[0-9]{2}.+(ERROR|INFO |TRACE|DEBUG|WARN ) ([a-z0-9_::-]+)\s+> (.+)")

RE_FAILED_TO_PARSE = re.compile("\[(.+)\] Failed to parse \(non-critical\) z2m bridge message on \[(.+)\]:$")

class Mode(enum.Enum):
    Idle = 0
    IgnoreObj = 1
    IgnoreLogo = 2
    Sample = 3
    SampleStart = 4

mode = Mode.Idle

res = []
buf = None
topic = None

BAR = "  ==================================================================="

for line in sys.stdin:
    line = line.rstrip()

    if not line:
        continue

    match mode:
        case Mode.Idle:
            if line == "{":
                mode = Mode.IgnoreObj

            if line == BAR:
                mode = Mode.IgnoreLogo

            if (log := RE_LOG.match(line)):
                if (err := RE_FAILED_TO_PARSE.match(log.group(3))):
                    mode = Mode.SampleStart
                    buf = ""
                    topic = err.group(2)

        case Mode.IgnoreObj:
            if line == "}":
                mode = Mode.Idle

        case Mode.IgnoreLogo:
            if line == BAR:
                mode = Mode.Idle

        case Mode.SampleStart:
            if (log := RE_LOG.match(line)):
                mode = Mode.Sample
                buf = log.group(3)

        case Mode.Sample:
            if (log := RE_LOG.match(line)) or line == BAR:
                try:
                    val = json.loads(buf)
                    res.append((topic, val))
                except:
                    print(f"FAILED TO PARSE JSON: {buf}")
                mode = Mode.IgnoreLogo if line == BAR else Mode.Idle
                buf = None
                topic = None
            else:
                buf += line

for topic, data in res:
    print(json.dumps({"topic": topic, "payload": data}))
