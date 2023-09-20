# select image
FROM rust AS build

#RUN apt-get install openssl
# copy your source tree
WORKDIR /app
COPY . .

# build for release
RUN cargo build --release

FROM gcr.io/distroless/cc-debian11
COPY --from=build /app/target/release/jitencoder /
COPY wwwroot/ wwwroot/
ENTRYPOINT [ "/jitencoder" ]