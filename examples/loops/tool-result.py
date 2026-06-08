#!/usr/bin/env python3
import json
import sys


def main() -> int:
    request = json.load(sys.stdin)
    if request.get("done") is True:
        result = {
            "ok": True,
            "result": {
                "final_answer": request.get("final_answer", ""),
                "tool": request.get("tool", "direct"),
            },
        }
    else:
        result = {
            "ok": True,
            "result": {
                "observation": "offline fixture executed",
                "tool": request.get("tool", "fixture"),
                "args": request.get("args", {}),
            },
        }
    print(json.dumps(result, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
