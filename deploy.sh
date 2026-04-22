host-shell podman login ghcr.io
host-shell podman build -t martinhamel/veloinfo:latest .
host-shell podman tag martinhamel/veloinfo:latest martinhamel/veloinfo:temp
host-shell podman push martinhamel/veloinfo:temp ghcr.io/martinhamel/veloinfo:latest
host-shell podman rmi martinhamel/veloinfo:temp
