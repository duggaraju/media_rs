FROM rust as builder
WORKDIR /app
COPY . .
RUN cargo install --path .

FROM debian:bookworm-slim as ffmpeg
RUN apt-get update && apt-get install -y xz-utils wget
WORKDIR /ffmpeg
ADD https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz /
RUN tar xf /ffmpeg-release-amd64-static.tar.xz --strip=1 ffmpeg-6.0-amd64-static/ffmpeg ffmpeg-6.0-amd64-static/ffprobe
RUN wget https://github.com/shaka-project/shaka-packager/releases/download/v2.6.1/packager-linux-x64 -O packager
RUN chmod +x packager

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl-dev ca-certificates
COPY --from=builder /usr/local/cargo/bin/encoder /usr/local/bin/encoder
COPY --from=ffmpeg /ffmpeg /usr/local/bin/
ENTRYPOINT ["encoder"]