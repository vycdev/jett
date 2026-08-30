# Disposable benchmark runner

Build from the repository root:

```text
docker build -f benchmarks/sandbox/Dockerfile -t jett-bench:0.1 .
```

Validate repository-owned baselines with network disabled and bounded resources:

```text
docker run --rm --network none --memory 2g --cpus 2 --pids-limit 256 \
  --cap-drop ALL --security-opt no-new-privileges \
  jett-bench:0.1 baselines --allow-unsafe-local
```

For generated-code grading, use a fresh container per submission. Do not pass
the host environment, API key, Docker socket, home directory, or repository
credentials. Copy only the result JSON out after the process exits. The image
pins major/minor toolchains; the result row still records exact versions and the
repository revision. A publication image should also be tagged by immutable
image digest.

The container's writable layer is discarded by `--rm`. `--network none`,
capability removal, `no-new-privileges`, memory, CPU, PID, and per-command time
limits are all required; none is a substitute for keeping credentials outside
the container.
