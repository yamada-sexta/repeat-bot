# -- Build stage --
FROM rust:1.86-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app

# Cache dependency builds: copy manifests first
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Build the actual app
COPY src/ src/
RUN touch src/main.rs && cargo build --release

# -- Runtime stage --
FROM alpine:3.23

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/repeat-bot /usr/local/bin/repeat-bot

# Store the database in a volume-mountable location
ENV DATABASE_PATH=/data/repeat_bot.db
RUN mkdir /data

ENTRYPOINT ["repeat-bot"]
