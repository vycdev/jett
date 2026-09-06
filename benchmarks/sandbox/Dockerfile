FROM python:3.12-bookworm AS python_runtime
FROM node:24-bookworm AS node_runtime
FROM golang:1.24-bookworm AS go_runtime
FROM rust:1.93-bookworm

COPY --from=python_runtime /usr/local /usr/local
COPY --from=node_runtime /usr/local /usr/local
COPY --from=go_runtime /usr/local/go /usr/local/go

ENV PATH="/usr/local/go/bin:${PATH}"
RUN npm install --global typescript@5.8.2 pyright@1.1.405

WORKDIR /workspace
COPY . /workspace
RUN cargo build -q -p jett_cli \
    && useradd --create-home --uid 10001 benchmark \
    && chown -R benchmark:benchmark /workspace

USER benchmark
ENTRYPOINT ["python", "tools/jett_bench.py"]
CMD ["validate"]
