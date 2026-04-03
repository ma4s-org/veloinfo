flatpak-spawn --host podman login ghcr.io
flatpak-spawn --host podman build -t martinhamel/veloinfo:latest .
flatpak-spawn --host podman tag martinhamel/veloinfo:latest martinhamel/veloinfo:temp
flatpak-spawn --host podman push martinhamel/veloinfo:temp ghcr.io/martinhamel/veloinfo:latest
flatpak-spawn --host podman rmi martinhamel/veloinfo:temp
