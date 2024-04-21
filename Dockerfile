FROM rust:1.77.1

COPY ./ ./
RUN cargo build --release

CMD ["./target/release/url_shortener"]