FROM rust:1.77.2-slim-bookworm

ADD . /app

WORKDIR /app
RUN cargo build --release
RUN cargo install --path .

EXPOSE 7788

CMD ["bcproxy-rust"]
