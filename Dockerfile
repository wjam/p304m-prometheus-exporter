FROM rust:1.90.0 AS builder

# Set the working directory inside the container
WORKDIR /usr/src/app

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock ./

# Create an empty src directory to trick Cargo into thinking it's a valid Rust project
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build the dependencies without the actual source code to cache dependencies separately
RUN cargo build --release

COPY ./src ./src

# Make main.rs be modified later than the fake main.rs created earlier
RUN touch src/main.rs

ARG version="unset"

ENV VERSION=$version

RUN cargo build --release

FROM gcr.io/distroless/cc-debian12

COPY --from=builder /usr/src/app/target/release/p304m-prometheus-exporter /usr/local/bin/

HEALTHCHECK --interval=1m CMD ["/usr/local/bin/p304m-prometheus-exporter", "health"]

ENTRYPOINT ["/usr/local/bin/p304m-prometheus-exporter"]
CMD ["server"]
