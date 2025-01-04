podman login docker.io
podman build -t martinhamel/veloinfo .
podman push martinhamel/veloinfo  docker://docker.io/martinhamel/veloinfo