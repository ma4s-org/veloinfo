flatpak-spawn --host podman login docker.io
flatpak-spawn --host podman build -t martinhamel/veloinfo .
flatpak-spawn --host podman push martinhamel/veloinfo  docker://docker.io/martinhamel/veloinfo