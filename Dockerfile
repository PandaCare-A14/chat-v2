# Builder stage
FROM rust:1.86-slim as builder
WORKDIR /usr/src/pandacare-chat
RUN apt update && apt install -y libpq-dev pkg-config build-essential && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo install --path .

# Runner stage
FROM debian:stable as runner
RUN apt update && apt install -y ca-certificates libpq-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/pandacare-chat-v2 /usr/local/bin/pandacare-chat-v2
EXPOSE 8080
CMD ["pandacare-chat-v2"]