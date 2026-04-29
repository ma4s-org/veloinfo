podman login ghcr.io
podman build -t martinhamel/veloinfo:latest .
podman tag martinhamel/veloinfo:latest martinhamel/veloinfo:temp
podman push martinhamel/veloinfo:temp ghcr.io/martinhamel/veloinfo:latest
podman rmi martinhamel/veloinfo:temp
