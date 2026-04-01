flatpak-spawn --host podman login docker.io
flatpak-spawn --host podman build -t martinhamel/veloinfo:latest -t martinhamel/veloinfo:$(date +%Y%m%d%H%M%S) .
flatpak-spawn --host podman push docker://docker.io/martinhamel/veloinfo:latest
flatpak-spawn --host podman push docker://docker.io/martinhamel/veloinfo:$(date +%Y%m%d%H%M%S)