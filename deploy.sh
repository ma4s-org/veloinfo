flatpak-spawn --host podman login docker.io
flatpak-spawn --host podman build -t martinhamel/veloinfo:latest -t martinhamel/veloinfo:latest .
flatpak-spawn --host podman push docker.io/martinhamel/veloinfo:latest
