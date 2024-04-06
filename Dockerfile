FROM rust:1.75 as builder

WORKDIR /app

# Copy the Cargo.toml and Cargo.lock and create a dummy main.rs
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs

# Build only the dependencies to cache them
RUN cargo build --release

# Now, copy the actual source code and rebuild
# This step uses the cached dependencies
COPY . .
RUN touch src/main.rs && \
    cargo build --release

# Final stage
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get install -y libssl-dev ca-certificates && \
    update-ca-certificates

# Copy the compiled binary and any other necessary files
COPY --from=builder /app/target/release/short-link /usr/local/bin/short-link
COPY --from=builder /app/links.json /usr/local/bin/links.json


EXPOSE 5008

CMD ["/usr/local/bin/short-link"]
