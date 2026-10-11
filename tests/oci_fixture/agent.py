"""Deliberately small external executable for testing the host OCI boundary.

This is not a real AI system. It has no authority beyond the request/response
protocol and runs in a disposable, unprivileged, read-only container.
"""
import json
import os
import pathlib
import socket
import sys
import time


def inspect() -> dict:
    writable_root = False
    try:
        pathlib.Path("/usr/local/bin/got-escape-attempt").write_text("escape", encoding="utf-8")
        writable_root = True
    except OSError:
        pass

    network_reachable = False
    try:
        with socket.create_connection(("1.1.1.1", 443), timeout=1):
            network_reachable = True
    except OSError:
        pass

    def cgroup_value(name: str) -> str:
        try:
            return pathlib.Path("/sys/fs/cgroup", name).read_text().strip()
        except OSError:
            return "unavailable"

    return {
        "uid": os.getuid(),
        "network_reachable": network_reachable,
        "filesystem_write_succeeded": writable_root,
        "host_workspace_visible": pathlib.Path("/home/runner/work/GOT").exists(),
        "docker_socket_visible": pathlib.Path("/var/run/docker.sock").exists(),
        "net_namespace": os.readlink("/proc/self/ns/net"),
        "memory_max": cgroup_value("memory.max"),
        "cpu_max": cgroup_value("cpu.max"),
        "pids_max": cgroup_value("pids.max"),
    }


def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else "probe"
    if mode == "probe":
        print(json.dumps(inspect()))
    elif mode == "hang":
        time.sleep(15)
    elif mode == "overflow":
        sys.stdout.write("X" * 200000)
    elif mode == "decide":
        request = json.loads(sys.stdin.readline())
        if request.get("schema_version") != 1:
            raise ValueError("unsupported schema")
        # This is a programmed fixture, not an actual language model.
        # Its output will be captured by the host as an agent action REQUEST,
        # not as proof the action was executed.
        severity = request["environment"].get("cpu_throttle", 0.0)
        actions = ["request_resource_reallocation"] if severity > 50 else []
        print(json.dumps({"schema_version": 1, "requested_actions": actions}))
    elif mode == "invalid":
        print("this-is-not-json")
    else:
        raise ValueError("unknown fixture mode")


if __name__ == "__main__":
    main()
